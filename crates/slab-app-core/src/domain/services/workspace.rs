use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::domain::models::{
    WorkspaceConsoleOutput, WorkspaceGitFileStatus, WorkspaceGitOperationView,
    WorkspaceGitStatusEntry, WorkspaceGitStatusSummary, WorkspaceGitStatusView,
    WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
use crate::error::AppCoreError;

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONSOLE_COMMAND_BYTES: usize = 2_000;
const MAX_CONSOLE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_WRITE_FILE_BYTES: usize = 2 * 1024 * 1024;

pub struct WorkspaceService;

impl WorkspaceService {
    pub fn write_file(
        root: impl AsRef<Path>,
        command: WorkspaceWriteFileCommand,
    ) -> Result<WorkspaceWriteFileView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(&command.relative_path)?;
        if relative_path.is_empty() {
            return Err(AppCoreError::BadRequest("file path cannot be empty".to_string()));
        }
        let bytes = command.content.as_bytes();
        if bytes.len() > MAX_WRITE_FILE_BYTES {
            return Err(AppCoreError::BadRequest(format!(
                "file is too large to save (limit {MAX_WRITE_FILE_BYTES} bytes)"
            )));
        }

        let path = resolve_workspace_path_for_write(root, &relative_path)?;
        if path.exists() {
            let metadata = fs::metadata(&path).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to read file metadata {}: {error}",
                    path.display()
                ))
            })?;
            if !metadata.is_file() {
                return Err(AppCoreError::BadRequest(format!(
                    "workspace path `{relative_path}` is not a file"
                )));
            }

            if let Some(expected_hash) = command.expected_hash.as_deref() {
                let current_bytes = fs::read(&path).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to read file {} before saving: {error}",
                        path.display()
                    ))
                })?;
                let current_hash = content_hash(&current_bytes);
                if current_hash != expected_hash {
                    return Err(AppCoreError::BadRequest(
                        "file changed on disk; reload before saving".to_string(),
                    ));
                }
            }
        } else if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create directory {}: {error}",
                    parent.display()
                ))
            })?;
        }

        fs::write(&path, bytes).map_err(|error| {
            AppCoreError::Internal(format!("failed to write file {}: {error}", path.display()))
        })?;

        Ok(WorkspaceWriteFileView {
            relative_path,
            size_bytes: bytes.len() as u64,
            content_hash: content_hash(bytes),
        })
    }

    pub fn git_status(root: impl AsRef<Path>) -> Result<WorkspaceGitStatusView, AppCoreError> {
        let root = root.as_ref();
        let status_output = match git_output(root, &["status", "--porcelain=v1", "-b"]) {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(WorkspaceGitStatusView {
                    available: false,
                    message: Some("Git is not available on PATH.".to_string()),
                    ..WorkspaceGitStatusView::default()
                });
            }
            Err(error) => {
                return Err(AppCoreError::Internal(format!(
                    "failed to run git status in {}: {error}",
                    root.display()
                )));
            }
        };

        if !status_output.status.success() {
            let message = String::from_utf8_lossy(&status_output.stderr).trim().to_owned();
            return Ok(WorkspaceGitStatusView {
                available: true,
                message: Some(if message.is_empty() {
                    "The workspace is not a Git repository.".to_string()
                } else {
                    message
                }),
                ..WorkspaceGitStatusView::default()
            });
        }

        let repository_root = git_output(root, &["rev-parse", "--show-toplevel"])
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| {
                let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                (!root.is_empty()).then_some(root)
            });
        let raw = String::from_utf8_lossy(&status_output.stdout);
        Ok(parse_git_status(&raw, repository_root))
    }

    pub fn git_stage(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(path)?;
        run_git_operation(root, &["add", "--", relative_path.as_str()])?;
        Ok(WorkspaceGitOperationView { status: Self::git_status(root)? })
    }

    pub fn git_unstage(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(path)?;
        run_git_operation(root, &["restore", "--staged", "--", relative_path.as_str()])?;
        Ok(WorkspaceGitOperationView { status: Self::git_status(root)? })
    }

    pub fn git_discard(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(path)?;
        run_git_operation(
            root,
            &["restore", "--staged", "--worktree", "--", relative_path.as_str()],
        )
        .or_else(|_| run_git_operation(root, &["clean", "-fd", "--", relative_path.as_str()]))?;
        Ok(WorkspaceGitOperationView { status: Self::git_status(root)? })
    }

    pub fn git_commit(
        root: impl AsRef<Path>,
        message: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        let root = root.as_ref();
        let message = message.trim();
        if message.is_empty() {
            return Err(AppCoreError::BadRequest("commit message cannot be empty".to_string()));
        }
        let status = Self::git_status(root)?;
        if should_stage_all_before_commit(&status) {
            run_git_operation(root, &["add", "--all"])?;
        }
        run_git_operation(root, &["commit", "-m", message])?;
        let status = Self::git_status(root)?;
        if should_push_after_commit(&status) {
            run_git_operation(root, &["push"])?;
            return Ok(WorkspaceGitOperationView { status: Self::git_status(root)? });
        }
        Ok(WorkspaceGitOperationView { status })
    }

    pub async fn run_console_command(
        root: impl AsRef<Path>,
        command: &str,
    ) -> Result<WorkspaceConsoleOutput, AppCoreError> {
        let command = command.trim();
        if command.is_empty() {
            return Err(AppCoreError::BadRequest("command cannot be empty".to_string()));
        }
        if command.len() > MAX_CONSOLE_COMMAND_BYTES {
            return Err(AppCoreError::BadRequest(format!(
                "command is too long (limit {MAX_CONSOLE_COMMAND_BYTES} bytes)"
            )));
        }

        let mut process = shell_command(command);
        process.current_dir(root.as_ref());
        process.stdout(Stdio::piped());
        process.stderr(Stdio::piped());
        process.kill_on_drop(true);

        let output = match timeout(CONSOLE_TIMEOUT, process.output()).await {
            Ok(result) => result.map_err(|error| {
                AppCoreError::Internal(format!("failed to run workspace command: {error}"))
            })?,
            Err(_) => {
                return Ok(WorkspaceConsoleOutput {
                    command: command.to_string(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!(
                        "Command timed out after {} seconds.",
                        CONSOLE_TIMEOUT.as_secs()
                    ),
                    timed_out: true,
                });
            }
        };

        Ok(WorkspaceConsoleOutput {
            command: command.to_string(),
            exit_code: output.status.code(),
            stdout: decode_limited_output(&output.stdout),
            stderr: decode_limited_output(&output.stderr),
            timed_out: false,
        })
    }
}

fn git_output(root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("git").arg("-C").arg(root).args(args).output()
}

fn run_git_operation(root: &Path, args: &[&str]) -> Result<(), AppCoreError> {
    let output = git_output(root, args)
        .map_err(|error| AppCoreError::Internal(format!("failed to run git: {error}")))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() { stderr } else { stdout };
    Err(AppCoreError::BadRequest(if message.is_empty() {
        "git operation failed".to_string()
    } else {
        message
    }))
}

#[cfg(windows)]
fn shell_command(command: &str) -> TokioCommand {
    let mut process = TokioCommand::new("powershell.exe");
    process.args(["-NoLogo", "-NoProfile", "-Command", command]);
    process
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> TokioCommand {
    let mut process = TokioCommand::new("sh");
    process.args(["-lc", command]);
    process
}

fn parse_git_status(raw: &str, repository_root: Option<String>) -> WorkspaceGitStatusView {
    let mut branch = None;
    let mut entries = Vec::new();
    let mut summary = WorkspaceGitStatusSummary::default();

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

    WorkspaceGitStatusView {
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

fn parse_status_line(line: &str) -> Option<WorkspaceGitStatusEntry> {
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
    let (original_path, path) = if matches!(status, WorkspaceGitFileStatus::Renamed)
        || matches!(status, WorkspaceGitFileStatus::Copied)
    {
        match path.split_once(" -> ") {
            Some((from, to)) => (Some(from.to_string()), to.to_string()),
            None => (None, path.to_string()),
        }
    } else {
        (None, path.to_string())
    };

    Some(WorkspaceGitStatusEntry { path, original_path, status, staged })
}

fn classify_status(code: &str) -> WorkspaceGitFileStatus {
    let mut chars = code.chars();
    let left = chars.next().unwrap_or(' ');
    let right = chars.next().unwrap_or(' ');

    if code == "??" {
        return WorkspaceGitFileStatus::Untracked;
    }
    if left == 'U' || right == 'U' || (left == 'A' && right == 'A') || (left == 'D' && right == 'D')
    {
        return WorkspaceGitFileStatus::Conflicted;
    }
    if left == 'R' || right == 'R' {
        return WorkspaceGitFileStatus::Renamed;
    }
    if left == 'C' || right == 'C' {
        return WorkspaceGitFileStatus::Copied;
    }
    if left == 'A' || right == 'A' {
        return WorkspaceGitFileStatus::Added;
    }
    if left == 'D' || right == 'D' {
        return WorkspaceGitFileStatus::Deleted;
    }

    WorkspaceGitFileStatus::Modified
}

fn increment_summary(summary: &mut WorkspaceGitStatusSummary, status: WorkspaceGitFileStatus) {
    match status {
        WorkspaceGitFileStatus::Added => summary.added += 1,
        WorkspaceGitFileStatus::Modified => summary.modified += 1,
        WorkspaceGitFileStatus::Deleted => summary.deleted += 1,
        WorkspaceGitFileStatus::Renamed => summary.renamed += 1,
        WorkspaceGitFileStatus::Copied => summary.copied += 1,
        WorkspaceGitFileStatus::Untracked => summary.untracked += 1,
        WorkspaceGitFileStatus::Conflicted => summary.conflicted += 1,
    }
}

fn should_stage_all_before_commit(status: &WorkspaceGitStatusView) -> bool {
    !status.entries.is_empty() && status.entries.iter().all(|entry| !entry.staged)
}

fn should_push_after_commit(status: &WorkspaceGitStatusView) -> bool {
    status.available && status.is_repository && status.entries.is_empty()
}

fn decode_limited_output(bytes: &[u8]) -> String {
    if bytes.len() <= MAX_CONSOLE_OUTPUT_BYTES {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    let mut output = String::from_utf8_lossy(&bytes[..MAX_CONSOLE_OUTPUT_BYTES]).into_owned();
    output.push_str("\n[output truncated]\n");
    output
}

fn normalize_relative_path(raw: &str) -> Result<String, AppCoreError> {
    let trimmed = raw.trim().trim_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment == ".slab" {
                    return Err(AppCoreError::BadRequest(
                        "workspace internals cannot be edited from the file tree".to_string(),
                    ));
                }
                parts.push(segment.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppCoreError::BadRequest(format!("workspace path `{raw}` is invalid")));
            }
        }
    }

    Ok(parts.join("/"))
}

fn resolve_workspace_path_for_write(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, AppCoreError> {
    let canonical_root = root.canonicalize().map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to resolve workspace root {}: {error}",
            root.display()
        ))
    })?;
    let candidate = canonical_root.join(relative_path);
    if let Some(parent) = candidate.parent() {
        let canonical_parent = existing_parent(parent)?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` escapes the workspace root"
            )));
        }
    }
    if candidate.exists() {
        let canonical_candidate = candidate.canonicalize().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to resolve workspace path {}: {error}",
                candidate.display()
            ))
        })?;
        if !canonical_candidate.starts_with(&canonical_root) {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` escapes the workspace root"
            )));
        }
    }

    Ok(candidate)
}

fn existing_parent(path: &Path) -> Result<PathBuf, AppCoreError> {
    let mut current = path;
    while !current.exists() {
        current = current.parent().ok_or_else(|| {
            AppCoreError::BadRequest("workspace path has no existing parent".to_string())
        })?;
    }
    current.canonicalize().map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to resolve workspace parent {}: {error}",
            current.display()
        ))
    })
}

fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_relative_path, parse_branch_line, parse_git_status, should_push_after_commit,
        should_stage_all_before_commit,
    };
    use crate::domain::models::{
        WorkspaceGitFileStatus, WorkspaceGitStatusEntry, WorkspaceGitStatusView,
    };

    #[test]
    fn parses_branch_with_tracking_status() {
        assert_eq!(parse_branch_line("## main...origin/main [ahead 1]"), Some("main".to_string()));
    }

    #[test]
    fn parses_git_status_entries_and_summary() {
        let status = parse_git_status(
            "## feature/workspace\n M src/main.rs\nA  added.ts\nR  old.md -> new.md\n?? scratch.txt\nUU conflict.ts\n",
            Some("C:/repo".to_string()),
        );

        assert!(status.available);
        assert!(status.is_repository);
        assert_eq!(status.branch.as_deref(), Some("feature/workspace"));
        assert_eq!(status.repository_root.as_deref(), Some("C:/repo"));
        assert_eq!(status.summary.modified, 1);
        assert_eq!(status.summary.added, 1);
        assert_eq!(status.summary.renamed, 1);
        assert_eq!(status.summary.untracked, 1);
        assert_eq!(status.summary.conflicted, 1);
        assert_eq!(status.entries[2].path, "new.md");
        assert_eq!(status.entries[2].original_path.as_deref(), Some("old.md"));
        assert_eq!(status.entries[2].status, WorkspaceGitFileStatus::Renamed);
        assert!(status.entries[1].staged);
        assert!(!status.entries[3].staged);
    }

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../outside.txt").is_err());
    }

    #[test]
    fn commit_auto_stages_only_when_index_is_empty() {
        let unstaged_status = WorkspaceGitStatusView {
            entries: vec![WorkspaceGitStatusEntry {
                path: "src/main.rs".to_string(),
                original_path: None,
                status: WorkspaceGitFileStatus::Modified,
                staged: false,
            }],
            ..WorkspaceGitStatusView::default()
        };
        let mixed_status = WorkspaceGitStatusView {
            entries: vec![
                WorkspaceGitStatusEntry {
                    path: "src/main.rs".to_string(),
                    original_path: None,
                    status: WorkspaceGitFileStatus::Modified,
                    staged: false,
                },
                WorkspaceGitStatusEntry {
                    path: "README.md".to_string(),
                    original_path: None,
                    status: WorkspaceGitFileStatus::Modified,
                    staged: true,
                },
            ],
            ..WorkspaceGitStatusView::default()
        };

        assert!(should_stage_all_before_commit(&unstaged_status));
        assert!(!should_stage_all_before_commit(&mixed_status));
        assert!(!should_stage_all_before_commit(&WorkspaceGitStatusView::default()));
    }

    #[test]
    fn commit_pushes_only_after_clean_commit_status() {
        let clean_repository = WorkspaceGitStatusView {
            available: true,
            is_repository: true,
            ..WorkspaceGitStatusView::default()
        };
        let dirty_repository = WorkspaceGitStatusView {
            available: true,
            is_repository: true,
            entries: vec![WorkspaceGitStatusEntry {
                path: "src/main.rs".to_string(),
                original_path: None,
                status: WorkspaceGitFileStatus::Modified,
                staged: false,
            }],
            ..WorkspaceGitStatusView::default()
        };
        let unavailable_git = WorkspaceGitStatusView {
            available: false,
            is_repository: true,
            ..WorkspaceGitStatusView::default()
        };

        assert!(should_push_after_commit(&clean_repository));
        assert!(!should_push_after_commit(&dirty_repository));
        assert!(!should_push_after_commit(&unavailable_git));
        assert!(!should_push_after_commit(&WorkspaceGitStatusView::default()));
    }
}
