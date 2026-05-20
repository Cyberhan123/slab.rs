//! Git helpers shared by Slab agent tools.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_GIT_DIFF_BYTES: usize = 256 * 1024;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git is not available on PATH")]
    GitUnavailable,
    #[error("not a Git repository: {0}")]
    NotRepository(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusSummary {
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub copied: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusEntry {
    pub path: String,
    pub original_path: Option<String>,
    pub status: GitFileStatus,
    pub staged: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatus {
    pub available: bool,
    pub is_repository: bool,
    pub branch: Option<String>,
    pub repository_root: Option<String>,
    pub message: Option<String>,
    pub summary: GitStatusSummary,
    pub entries: Vec<GitStatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitDiff {
    pub path: Option<String>,
    pub staged: bool,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitCommitResult {
    pub status: GitStatus,
}

pub struct GitRepository {
    root: PathBuf,
}

impl GitRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn status(&self) -> Result<GitStatus, GitError> {
        let status_output = match git_output(&self.root, &["status", "--porcelain=v1", "-b"]) {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(GitStatus {
                    available: false,
                    message: Some("Git is not available on PATH.".to_string()),
                    ..GitStatus::default()
                });
            }
            Err(error) => return Err(GitError::Io(error)),
        };

        if !status_output.status.success() {
            let message = output_message(&status_output);
            return Ok(GitStatus {
                available: true,
                message: Some(if message.is_empty() {
                    "The workspace is not a Git repository.".to_string()
                } else {
                    message
                }),
                ..GitStatus::default()
            });
        }

        let repository_root = git_output(&self.root, &["rev-parse", "--show-toplevel"])
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                (!root.is_empty()).then_some(root)
            });

        let raw = String::from_utf8_lossy(&status_output.stdout);
        Ok(parse_git_status(&raw, repository_root))
    }

    pub fn diff(&self, path: Option<&str>, staged: bool) -> Result<GitDiff, GitError> {
        let relative_path = path.map(validate_relative_path).transpose()?;
        let mut args = vec!["diff"];
        if staged {
            args.push("--cached");
        }
        if let Some(path) = relative_path.as_deref() {
            args.extend(["--", path]);
        }
        let output = git_output(&self.root, &args)?;
        if !output.status.success() {
            return Err(GitError::CommandFailed(output_message(&output)));
        }

        let mut diff = decode_limited_output(&output.stdout, MAX_GIT_DIFF_BYTES);
        if diff.trim().is_empty()
            && !staged
            && let Some(path) = relative_path.as_deref()
            && is_untracked_git_path(&self.root, path)?
        {
            diff = git_untracked_file_diff(&self.root, path)?;
        }

        Ok(GitDiff { path: relative_path, staged, diff })
    }

    pub fn commit_all(&self, message: &str) -> Result<GitCommitResult, GitError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(GitError::CommandFailed("commit message cannot be empty".to_string()));
        }
        run_git_operation(&self.root, &["add", "--all"])?;
        run_git_operation(&self.root, &["commit", "-m", message])?;
        Ok(GitCommitResult { status: self.status()? })
    }
}

fn validate_relative_path(path: &str) -> Result<String, GitError> {
    slab_file_system::normalize_relative_path(path)
        .map_err(|e| GitError::InvalidPath(e.to_string()))
}

fn git_output(root: &Path, args: &[&str]) -> std::io::Result<Output> {
    Command::new("git").arg("-C").arg(root).args(args).output()
}

fn run_git_operation(root: &Path, args: &[&str]) -> Result<(), GitError> {
    let output = git_output(root, args)?;
    if output.status.success() {
        return Ok(());
    }
    Err(GitError::CommandFailed(output_message(&output)))
}

fn is_untracked_git_path(root: &Path, relative_path: &str) -> Result<bool, GitError> {
    let output =
        git_output(root, &["ls-files", "--others", "--exclude-standard", "--", relative_path])?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(output_message(&output)));
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn git_untracked_file_diff(root: &Path, relative_path: &str) -> Result<String, GitError> {
    let output = git_output(root, &["diff", "--no-index", "--", "/dev/null", relative_path])?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(GitError::CommandFailed(output_message(&output)));
    }
    Ok(decode_limited_output(&output.stdout, MAX_GIT_DIFF_BYTES))
}

fn output_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn decode_limited_output(bytes: &[u8], limit: usize) -> String {
    if bytes.len() <= limit {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut output = String::from_utf8_lossy(&bytes[..limit]).into_owned();
    output.push_str("\n[output truncated]\n");
    output
}

fn parse_git_status(raw: &str, repository_root: Option<String>) -> GitStatus {
    let mut branch = None;
    let mut entries = Vec::new();
    let mut summary = GitStatusSummary::default();

    for line in raw.lines() {
        if let Some(parsed_branch) = parse_branch_line(line) {
            branch = Some(parsed_branch);
            continue;
        }
        let Some(entry) = parse_status_line(line) else {
            continue;
        };
        increment_summary(&mut summary, entry.status);
        entries.push(entry);
    }

    GitStatus {
        available: true,
        is_repository: true,
        branch,
        repository_root,
        message: None,
        summary,
        entries,
    }
}

fn parse_branch_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("## ")?;
    if let Some(branch) = rest.strip_prefix("No commits yet on ") {
        return Some(branch.to_string());
    }
    if rest == "HEAD (no branch)" {
        return Some("detached HEAD".to_string());
    }

    let without_tracking = rest.split("...").next().unwrap_or(rest);
    let without_ahead = without_tracking.split(" [").next().unwrap_or(without_tracking);
    let branch = without_ahead.trim();
    (!branch.is_empty()).then_some(branch.to_string())
}

fn parse_status_line(line: &str) -> Option<GitStatusEntry> {
    if line.len() < 4 {
        return None;
    }

    let code = &line[..2];
    let staged = code.chars().next().is_some_and(|status| status != ' ' && status != '?');
    let path = line[3..].trim();
    if path.is_empty() {
        return None;
    }

    let status = classify_status(code);
    let (original_path, path) = if matches!(status, GitFileStatus::Renamed | GitFileStatus::Copied)
    {
        match path.split_once(" -> ") {
            Some((from, to)) => (Some(from.to_string()), to.to_string()),
            None => (None, path.to_string()),
        }
    } else {
        (None, path.to_string())
    };

    Some(GitStatusEntry { path, original_path, status, staged })
}

fn classify_status(code: &str) -> GitFileStatus {
    let mut chars = code.chars();
    let left = chars.next().unwrap_or(' ');
    let right = chars.next().unwrap_or(' ');

    if code == "??" {
        return GitFileStatus::Untracked;
    }
    if left == 'U' || right == 'U' || (left == 'A' && right == 'A') || (left == 'D' && right == 'D')
    {
        return GitFileStatus::Conflicted;
    }
    if left == 'R' || right == 'R' {
        return GitFileStatus::Renamed;
    }
    if left == 'C' || right == 'C' {
        return GitFileStatus::Copied;
    }
    if left == 'A' || right == 'A' {
        return GitFileStatus::Added;
    }
    if left == 'D' || right == 'D' {
        return GitFileStatus::Deleted;
    }

    GitFileStatus::Modified
}

fn increment_summary(summary: &mut GitStatusSummary, status: GitFileStatus) {
    match status {
        GitFileStatus::Added => summary.added += 1,
        GitFileStatus::Modified => summary.modified += 1,
        GitFileStatus::Deleted => summary.deleted += 1,
        GitFileStatus::Renamed => summary.renamed += 1,
        GitFileStatus::Copied => summary.copied += 1,
        GitFileStatus::Untracked => summary.untracked += 1,
        GitFileStatus::Conflicted => summary.conflicted += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn parses_status_entries() {
        let status = parse_git_status(
            "## main...origin/main [ahead 1]\n M src/main.rs\nA  added.ts\nR  old.md -> new.md\n?? scratch.txt\n",
            Some("C:/repo".to_string()),
        );
        assert!(status.available);
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.summary.modified, 1);
        assert_eq!(status.summary.added, 1);
        assert_eq!(status.summary.renamed, 1);
        assert_eq!(status.summary.untracked, 1);
        assert_eq!(status.entries[2].original_path.as_deref(), Some("old.md"));
    }

    #[test]
    fn status_diff_and_commit_all_in_temp_repo() {
        let root = temp_root("repo");
        if run_git(&root, &["init"]).is_none() {
            let _ = fs::remove_dir_all(root);
            return;
        }
        run_git(&root, &["config", "user.email", "agent@example.test"]).expect("config email");
        run_git(&root, &["config", "user.name", "Slab Agent"]).expect("config name");
        fs::write(root.join("hello.txt"), "hello\n").expect("write file");

        let repo = GitRepository::new(&root);
        let status = repo.status().expect("status should work");
        assert!(status.available);
        assert!(status.is_repository);
        assert_eq!(status.summary.untracked, 1);

        let diff = repo.diff(Some("hello.txt"), false).expect("diff should work");
        assert!(diff.diff.contains("hello"));

        let result = repo.commit_all("initial commit").expect("commit should work");
        assert!(result.status.entries.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    fn run_git(root: &Path, args: &[&str]) -> Option<Output> {
        Command::new("git").arg("-C").arg(root).args(args).output().ok()
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root =
            std::env::temp_dir().join(format!("slab_git_{name}_{}_{}", std::process::id(), nonce));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }
}
