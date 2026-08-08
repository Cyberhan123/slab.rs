use std::{
    collections::BTreeMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use once_cell::sync::Lazy;
use regex::Regex;
use similar::{ChangeTag, TextDiff};
use slab_sandboxing::{SandboxDriver, SandboxedOutput, spawn_sandboxed_option};
use slab_utils::string::{decode_truncated_output, truncate_output_prefix};
use tracing::debug;
use walkdir::WalkDir;

use crate::types::{
    GitCommitOptions, GitCommitResult, GitDiff, GitError, GitFileStatus, GitOperationResult,
    GitPathDiff, GitRepositoryMetadata, GitStatus, GitStatusEntry, GitStatusSummary,
};

const MAX_GIT_DIFF_BYTES: usize = 256 * 1024;

static GIT_INTERNAL_PATH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(^|/)\.git(/|$)").expect("valid git internal path regex"));

pub struct GitRepository {
    root: PathBuf,
    driver: Option<Arc<dyn SandboxDriver>>,
}

impl GitRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), driver: None }
    }

    /// Construct with an explicit sandbox driver (agent-driven path). `None`
    /// falls back to `PassThroughDriver` — the current host behavior (no
    /// isolation, but spawn/timeout/tree-kill still owned by the driver).
    pub fn new_with_driver(
        root: impl Into<PathBuf>,
        driver: Option<Arc<dyn SandboxDriver>>,
    ) -> Self {
        Self { root: root.into(), driver }
    }

    pub fn metadata(&self) -> Result<Option<GitRepositoryMetadata>, GitError> {
        let repo = match gix::discover(&self.root) {
            Ok(repo) => repo,
            Err(error) => {
                let message = error.to_string();
                if message.contains("Could not find repository")
                    || message.contains("not a git repository")
                {
                    return Ok(None);
                }
                return Err(GitError::RepositoryDiscovery(message));
            }
        };

        let repository_root =
            repo.workdir().unwrap_or_else(|| repo.path()).to_string_lossy().into_owned();
        let git_dir = repo.path().to_string_lossy().into_owned();
        let branch = repo
            .head_name()
            .map_err(|error| GitError::RepositoryDiscovery(error.to_string()))?
            .map(|name| name.shorten().to_string());

        Ok(Some(GitRepositoryMetadata {
            repository_root,
            git_dir,
            branch,
            discovered_at: chrono::Utc::now().to_rfc3339(),
        }))
    }

    pub async fn status(&self) -> Result<GitStatus, GitError> {
        let porcelain =
            match git_output(self.driver.as_ref(), &self.root, &["status", "--porcelain=v1", "-b"])
                .await
            {
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

        if !porcelain.success() {
            let message = output_message(&porcelain);
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

        let mut metadata = self.metadata()?;
        if metadata.is_none() {
            metadata = metadata_from_git(self.driver.as_ref(), &self.root).await;
        }
        if let Err(error) = validate_status_with_gix(&self.root) {
            debug!(%error, "gix status read failed; using git porcelain status");
        }

        let raw = String::from_utf8_lossy(&porcelain.stdout);
        Ok(parse_git_status(
            &raw,
            metadata.as_ref().map(|metadata| metadata.repository_root.clone()),
            metadata.and_then(|metadata| metadata.branch),
        ))
    }

    pub async fn stage(&self, path: &str) -> Result<GitOperationResult, GitError> {
        let relative_path = validate_relative_path(path)?;
        run_git_operation(self.driver.as_ref(), &self.root, &["add", "--", relative_path.as_str()])
            .await?;
        Ok(GitOperationResult { status: self.status().await? })
    }

    pub async fn unstage(&self, path: &str) -> Result<GitOperationResult, GitError> {
        let relative_path = validate_relative_path(path)?;
        run_git_operation(
            self.driver.as_ref(),
            &self.root,
            &["restore", "--staged", "--", relative_path.as_str()],
        )
        .await?;
        Ok(GitOperationResult { status: self.status().await? })
    }

    pub async fn discard(&self, path: &str) -> Result<GitOperationResult, GitError> {
        let relative_path = validate_relative_path(path)?;
        // `restore` revives a deleted file from the index; if that fails (e.g.
        // untracked file with no index entry), fall back to `clean -fd`.
        if run_git_operation(
            self.driver.as_ref(),
            &self.root,
            &["restore", "--staged", "--worktree", "--", relative_path.as_str()],
        )
        .await
        .is_err()
        {
            run_git_operation(
                self.driver.as_ref(),
                &self.root,
                &["clean", "-fd", "--", relative_path.as_str()],
            )
            .await?;
        }
        Ok(GitOperationResult { status: self.status().await? })
    }

    pub async fn diff(&self, path: Option<&str>, staged: bool) -> Result<GitDiff, GitError> {
        let relative_path = path.map(validate_relative_path).transpose()?;
        let mut args = vec!["diff"];
        if staged {
            args.push("--cached");
        }
        if let Some(path) = relative_path.as_deref() {
            args.extend(["--", path]);
        }
        let output = git_output(self.driver.as_ref(), &self.root, &args).await?;
        if !output.success() {
            return Err(GitError::CommandFailed(output_message(&output)));
        }

        let mut diff = decode_truncated_output(&output.stdout, MAX_GIT_DIFF_BYTES);
        if diff.trim().is_empty()
            && !staged
            && let Some(path) = relative_path.as_deref()
            && is_untracked_git_path(self.driver.as_ref(), &self.root, path).await?
        {
            diff = untracked_path_diff(&self.root, path)?;
        }

        Ok(GitDiff { path: relative_path, staged, diff })
    }

    pub async fn path_diff(&self, path: &str, staged: bool) -> Result<GitPathDiff, GitError> {
        let diff = self.diff(Some(path), staged).await?;
        let path = diff.path.unwrap_or_default();
        let (original_content, modified_content) =
            path_diff_contents(self.driver.as_ref(), &self.root, &path, staged).await?;
        Ok(GitPathDiff {
            path,
            staged: diff.staged,
            diff: diff.diff,
            original_content,
            modified_content,
        })
    }

    pub async fn commit(
        &self,
        message: &str,
        options: GitCommitOptions,
    ) -> Result<GitCommitResult, GitError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(GitError::CommandFailed("commit message cannot be empty".to_string()));
        }

        let status = self.status().await?;
        if options.auto_stage_when_index_empty && should_stage_all_before_commit(&status) {
            run_git_operation(self.driver.as_ref(), &self.root, &["add", "--all"]).await?;
        }

        run_git_operation(self.driver.as_ref(), &self.root, &["commit", "-m", message]).await?;
        let status = self.status().await?;
        if options.push_after_clean_commit && should_push_after_commit(&status) {
            run_git_operation(self.driver.as_ref(), &self.root, &["push"]).await?;
            return Ok(GitCommitResult { status: self.status().await? });
        }

        Ok(GitCommitResult { status })
    }

    pub async fn commit_all(&self, message: &str) -> Result<GitCommitResult, GitError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(GitError::CommandFailed("commit message cannot be empty".to_string()));
        }
        run_git_operation(self.driver.as_ref(), &self.root, &["add", "--all"]).await?;
        self.commit(message, GitCommitOptions::agent_default()).await
    }
}

fn validate_relative_path(path: &str) -> Result<String, GitError> {
    let normalized = slab_file::normalize_relative_path(path)
        .map_err(|error| GitError::InvalidPath(error.to_string()))?;
    if GIT_INTERNAL_PATH.is_match(&normalized) {
        return Err(GitError::InvalidPath("Git internals cannot be edited".to_string()));
    }
    Ok(normalized)
}

async fn git_output(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
    args: &[&str],
) -> std::io::Result<SandboxedOutput> {
    let mut argv: Vec<String> =
        vec!["git".to_owned(), "-C".to_owned(), root.to_string_lossy().into_owned()];
    argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    spawn_sandboxed_option(driver, argv, Some(root.to_path_buf()))
        .await
        .map_err(sandbox_error_to_io)
}

/// Map a driver error back to `std::io::Error`. A spawn failure for a missing
/// binary (git not on PATH) is reported as `ErrorKind::NotFound` so callers like
/// [`GitRepository::status`] can degrade gracefully instead of hard-erroring.
fn sandbox_error_to_io(error: slab_sandboxing::SandboxError) -> std::io::Error {
    use slab_sandboxing::SandboxError;
    if let SandboxError::SpawnFailed(message) = &error {
        let lower = message.to_ascii_lowercase();
        if lower.contains("os error 2") || lower.contains("not found") {
            return std::io::Error::from_raw_os_error(2);
        }
    }
    std::io::Error::other(error.to_string())
}

async fn run_git_operation(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
    args: &[&str],
) -> Result<(), GitError> {
    let output = git_output(driver, root, args).await?;
    if output.success() {
        return Ok(());
    }
    Err(GitError::CommandFailed(output_message(&output)))
}

async fn metadata_from_git(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
) -> Option<GitRepositoryMetadata> {
    let root_output = git_output(driver, root, &["rev-parse", "--show-toplevel"]).await.ok()?;
    if !root_output.success() {
        return None;
    }
    let repository_root = String::from_utf8_lossy(&root_output.stdout).trim().to_owned();
    if repository_root.is_empty() {
        return None;
    }

    let git_dir = git_output(driver, root, &["rev-parse", "--git-dir"])
        .await
        .ok()
        .filter(|output| output.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| Path::new(&repository_root).join(".git").to_string_lossy().into_owned());

    Some(GitRepositoryMetadata {
        branch: None,
        repository_root,
        git_dir,
        discovered_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn validate_status_with_gix(root: &Path) -> Result<(), GitError> {
    let repo =
        gix::discover(root).map_err(|error| GitError::RepositoryDiscovery(error.to_string()))?;
    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|error| GitError::RepositoryStatus(error.to_string()))?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_index_worktree_iter(Vec::new())
        .map_err(|error| GitError::RepositoryStatus(error.to_string()))?;

    for item in iter {
        item.map_err(|error| GitError::RepositoryStatus(error.to_string()))?;
    }
    Ok(())
}

fn status_from_entries(
    repository_root: String,
    branch: Option<String>,
    entries: Vec<GitStatusEntry>,
) -> GitStatus {
    let mut merged: BTreeMap<(String, bool), GitStatusEntry> = BTreeMap::new();
    for entry in entries {
        merged.insert((entry.path.clone(), entry.staged), entry);
    }

    let mut entries: Vec<_> = merged.into_values().collect();
    entries.sort_by(|left, right| left.path.cmp(&right.path).then(left.staged.cmp(&right.staged)));

    let mut summary = GitStatusSummary::default();
    for entry in &entries {
        increment_summary(&mut summary, entry.status);
    }

    GitStatus {
        available: true,
        is_repository: true,
        branch,
        repository_root: Some(repository_root),
        message: None,
        summary,
        entries,
    }
}

async fn is_untracked_git_path(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
    relative_path: &str,
) -> Result<bool, GitError> {
    let output = git_output(
        driver,
        root,
        &["ls-files", "--others", "--exclude-standard", "--", relative_path],
    )
    .await?;
    if !output.success() {
        return Err(GitError::CommandFailed(output_message(&output)));
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn untracked_path_diff(root: &Path, relative_path: &str) -> Result<String, GitError> {
    let path = root.join(relative_path);
    let metadata = path.metadata()?;
    if metadata.is_dir() {
        return untracked_directory_diff(root, relative_path);
    }

    let content = std::fs::read_to_string(&path).unwrap_or_else(|_| String::new());
    Ok(render_added_file_diff(relative_path, &content))
}

async fn path_diff_contents(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
    relative_path: &str,
    staged: bool,
) -> Result<(String, String), GitError> {
    if !staged && is_untracked_git_path(driver, root, relative_path).await? {
        return Ok(("".to_string(), read_worktree_text(root, relative_path)?));
    }

    let original_revision =
        if staged { format!("HEAD:{relative_path}") } else { format!(":{relative_path}") };
    let original_content = read_git_blob(driver, root, &original_revision).await?;
    let modified_content = if staged {
        read_git_blob(driver, root, &format!(":{relative_path}")).await?
    } else {
        read_worktree_text(root, relative_path)?
    };

    Ok((original_content, modified_content))
}

async fn read_git_blob(
    driver: Option<&Arc<dyn SandboxDriver>>,
    root: &Path,
    revision: &str,
) -> Result<String, GitError> {
    let output = git_output(driver, root, &["cat-file", "-e", revision]).await?;
    if !output.success() {
        return Ok(String::new());
    }

    let output = git_output(driver, root, &["show", "--textconv", revision]).await?;
    if output.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(GitError::CommandFailed(output_message(&output)))
}

fn read_worktree_text(root: &Path, relative_path: &str) -> Result<String, GitError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(String::new());
    }
    if path.is_dir() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path).unwrap_or_default())
}

fn untracked_directory_diff(root: &Path, relative_path: &str) -> Result<String, GitError> {
    let mut output = String::new();
    for entry in WalkDir::new(root.join(relative_path)).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("failed to strip workspace root from {}", path.display()))
            .map_err(|error| GitError::InvalidPath(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let content = std::fs::read_to_string(path).unwrap_or_else(|_| String::new());
        output.push_str(&render_added_file_diff(&relative, &content));
    }
    Ok(truncate_output_prefix(output, MAX_GIT_DIFF_BYTES))
}

fn render_added_file_diff(relative_path: &str, content: &str) -> String {
    let diff = TextDiff::from_lines("", content);
    let mut output =
        format!("diff --git a/{relative_path} b/{relative_path}\nnew file mode 100644\n");
    output.push_str("--- /dev/null\n");
    output.push_str(&format!("+++ b/{relative_path}\n"));
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => output.push('-'),
            ChangeTag::Insert => output.push('+'),
            ChangeTag::Equal => output.push(' '),
        }
        output.push_str(change.value());
    }
    truncate_output_prefix(output, MAX_GIT_DIFF_BYTES)
}

fn output_message(output: &SandboxedOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn parse_git_status(
    raw: &str,
    repository_root: Option<String>,
    fallback_branch: Option<String>,
) -> GitStatus {
    let mut branch = fallback_branch;
    let mut entries = Vec::new();

    for line in raw.lines() {
        if let Some(parsed_branch) = parse_branch_line(line) {
            branch = Some(parsed_branch);
            continue;
        }
        let Some(entry) = parse_status_line(line) else {
            continue;
        };
        entries.push(entry);
    }

    status_from_entries(repository_root.unwrap_or_default(), branch, entries)
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

fn should_stage_all_before_commit(status: &GitStatus) -> bool {
    !status.entries.is_empty() && status.entries.iter().all(|entry| !entry.staged)
}

fn should_push_after_commit(status: &GitStatus) -> bool {
    status.available && status.is_repository && status.entries.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Output};
    use std::{fs, path::Path};

    #[test]
    fn parses_status_entries() {
        let status = parse_git_status(
            "## main...origin/main [ahead 1]\n M src/main.rs\nA  added.ts\nR  old.md -> new.md\n?? scratch.txt\n",
            Some("C:/repo".to_string()),
            None,
        );
        assert!(status.available);
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.summary.modified, 1);
        assert_eq!(status.summary.added, 1);
        assert_eq!(status.summary.renamed, 1);
        assert_eq!(status.summary.untracked, 1);
        let rename = status.entries.iter().find(|entry| entry.path == "new.md").expect("rename");
        assert_eq!(rename.original_path.as_deref(), Some("old.md"));
    }

    #[test]
    fn parses_branch_status_and_conflict_edge_cases() {
        let status = parse_git_status(
            "## No commits yet on main\nUU conflicted.txt\nC  copied.md -> copy.md\n D deleted.rs\n",
            Some("C:/repo".to_string()),
            Some("fallback".to_string()),
        );

        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.summary.conflicted, 1);
        assert_eq!(status.summary.copied, 1);
        assert_eq!(status.summary.deleted, 1);
        assert_eq!(
            status
                .entries
                .iter()
                .find(|entry| entry.path == "copy.md")
                .map(|entry| { (entry.original_path.as_deref(), entry.status, entry.staged) }),
            Some((Some("copied.md"), GitFileStatus::Copied, true))
        );
        assert_eq!(parse_branch_line("## HEAD (no branch)").as_deref(), Some("detached HEAD"));
        assert_eq!(
            parse_branch_line("## feature...origin/feature [behind 2]").as_deref(),
            Some("feature")
        );
    }

    #[test]
    fn rejects_git_internal_and_escaping_paths() {
        assert!(matches!(validate_relative_path(".git/config"), Err(GitError::InvalidPath(_))));
        assert!(matches!(validate_relative_path("src/.git/config"), Err(GitError::InvalidPath(_))));
        assert!(matches!(validate_relative_path("../outside"), Err(GitError::InvalidPath(_))));
        assert_eq!(validate_relative_path("src//main.rs").expect("normalized"), "src/main.rs");
    }

    #[test]
    fn commit_policy_auto_stages_only_when_index_is_empty() {
        let unstaged_status = GitStatus {
            entries: vec![GitStatusEntry {
                path: "src/main.rs".to_string(),
                original_path: None,
                status: GitFileStatus::Modified,
                staged: false,
            }],
            ..GitStatus::default()
        };
        let mixed_status = GitStatus {
            entries: vec![
                GitStatusEntry {
                    path: "src/main.rs".to_string(),
                    original_path: None,
                    status: GitFileStatus::Modified,
                    staged: false,
                },
                GitStatusEntry {
                    path: "README.md".to_string(),
                    original_path: None,
                    status: GitFileStatus::Modified,
                    staged: true,
                },
            ],
            ..GitStatus::default()
        };

        assert!(should_stage_all_before_commit(&unstaged_status));
        assert!(!should_stage_all_before_commit(&mixed_status));
        assert!(!should_stage_all_before_commit(&GitStatus::default()));
    }

    #[test]
    fn commit_policy_pushes_only_after_clean_commit_status() {
        let clean_repository =
            GitStatus { available: true, is_repository: true, ..GitStatus::default() };
        let dirty_repository = GitStatus {
            available: true,
            is_repository: true,
            entries: vec![GitStatusEntry {
                path: "src/main.rs".to_string(),
                original_path: None,
                status: GitFileStatus::Modified,
                staged: false,
            }],
            ..GitStatus::default()
        };
        let unavailable_git =
            GitStatus { available: false, is_repository: true, ..GitStatus::default() };

        assert!(should_push_after_commit(&clean_repository));
        assert!(!should_push_after_commit(&dirty_repository));
        assert!(!should_push_after_commit(&unavailable_git));
        assert!(!should_push_after_commit(&GitStatus::default()));
    }

    #[tokio::test]
    async fn status_stage_diff_and_commit_in_temp_repo() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        run_git(root.path(), &["config", "user.email", "agent@example.test"])
            .expect("config email");
        run_git(root.path(), &["config", "user.name", "Slab Agent"]).expect("config name");
        fs::write(root.path().join("hello.txt"), "hello\n").expect("write file");

        let repo = GitRepository::new(root.path());
        let status = repo.status().await.expect("status should work");
        assert!(status.available);
        assert!(status.is_repository);
        assert_eq!(status.summary.untracked, 1);

        let diff = repo.diff(Some("hello.txt"), false).await.expect("diff should work");
        assert!(diff.diff.contains("hello"));

        let staged = repo.stage("hello.txt").await.expect("stage should work");
        assert_eq!(staged.status.summary.added, 1);

        let result = repo.commit_all("initial commit").await.expect("commit should work");
        assert!(result.status.entries.is_empty());
    }

    #[tokio::test]
    async fn path_diff_contents_for_unstaged_tracked_file_use_index_and_worktree() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        run_git(root.path(), &["config", "user.email", "agent@example.test"])
            .expect("config email");
        run_git(root.path(), &["config", "user.name", "Slab Agent"]).expect("config name");
        fs::write(root.path().join("hello.txt"), "head\n").expect("write file");
        run_git(root.path(), &["add", "hello.txt"]).expect("stage file");
        run_git(root.path(), &["commit", "-m", "initial"]).expect("commit file");

        fs::write(root.path().join("hello.txt"), "index\n").expect("write index version");
        run_git(root.path(), &["add", "hello.txt"]).expect("stage index version");
        fs::write(root.path().join("hello.txt"), "worktree\n").expect("write worktree version");

        let diff = GitRepository::new(root.path())
            .path_diff("hello.txt", false)
            .await
            .expect("diff should work");
        assert_eq!(diff.original_content, "index\n");
        assert_eq!(diff.modified_content, "worktree\n");
    }

    #[tokio::test]
    async fn path_diff_contents_for_staged_file_use_head_and_index() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        run_git(root.path(), &["config", "user.email", "agent@example.test"])
            .expect("config email");
        run_git(root.path(), &["config", "user.name", "Slab Agent"]).expect("config name");
        fs::write(root.path().join("hello.txt"), "head\n").expect("write file");
        run_git(root.path(), &["add", "hello.txt"]).expect("stage file");
        run_git(root.path(), &["commit", "-m", "initial"]).expect("commit file");

        fs::write(root.path().join("hello.txt"), "index\n").expect("write index version");
        run_git(root.path(), &["add", "hello.txt"]).expect("stage index version");
        fs::write(root.path().join("hello.txt"), "worktree\n").expect("write worktree version");

        let diff = GitRepository::new(root.path())
            .path_diff("hello.txt", true)
            .await
            .expect("diff should work");
        assert_eq!(diff.original_content, "head\n");
        assert_eq!(diff.modified_content, "index\n");
    }

    #[tokio::test]
    async fn path_diff_contents_for_untracked_file_use_empty_original() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        fs::write(root.path().join("scratch.txt"), "scratch\n").expect("write file");

        let diff = GitRepository::new(root.path())
            .path_diff("scratch.txt", false)
            .await
            .expect("diff should work");
        assert_eq!(diff.original_content, "");
        assert_eq!(diff.modified_content, "scratch\n");
    }

    #[tokio::test]
    async fn path_diff_contents_for_deleted_file_use_empty_modified() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        run_git(root.path(), &["config", "user.email", "agent@example.test"])
            .expect("config email");
        run_git(root.path(), &["config", "user.name", "Slab Agent"]).expect("config name");
        fs::write(root.path().join("hello.txt"), "head\n").expect("write file");
        run_git(root.path(), &["add", "hello.txt"]).expect("stage file");
        run_git(root.path(), &["commit", "-m", "initial"]).expect("commit file");
        fs::remove_file(root.path().join("hello.txt")).expect("delete file");

        let diff = GitRepository::new(root.path())
            .path_diff("hello.txt", false)
            .await
            .expect("diff should work");
        assert_eq!(diff.original_content, "head\n");
        assert_eq!(diff.modified_content, "");
    }

    #[tokio::test]
    async fn untracked_directory_diff_uses_relative_file_paths() {
        let root = tempfile::tempdir().expect("temp repo");
        if run_git(root.path(), &["init"]).is_none() {
            return;
        }
        fs::create_dir_all(root.path().join("src")).expect("create dir");
        fs::write(root.path().join("src").join("main.rs"), "fn main() {}\n").expect("write file");

        let repo = GitRepository::new(root.path());
        let diff = repo.diff(Some("src"), false).await.expect("directory diff should work");
        assert!(diff.diff.contains("diff --git a/src/main.rs b/src/main.rs"));
        assert!(diff.diff.contains("+fn main() {}"));
    }

    fn run_git(root: &Path, args: &[&str]) -> Option<Output> {
        Command::new("git").arg("-C").arg(root).args(args).output().ok()
    }
}
