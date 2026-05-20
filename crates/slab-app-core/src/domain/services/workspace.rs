use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use sha2::{Digest, Sha256};
use slab_git::{GitCommitOptions, GitError, GitRepository};
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;

use crate::domain::models::{
    WorkspaceConsoleOutput, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceGitDiffView, WorkspaceGitOperationView,
    WorkspaceGitStatusView, WorkspacePathView, WorkspaceRenamePathCommand,
    WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
use crate::error::AppCoreError;

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONSOLE_COMMAND_BYTES: usize = 2_000;
const MAX_CONSOLE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_WRITE_FILE_BYTES: usize = 2 * 1024 * 1024;

pub struct WorkspaceService;

impl WorkspaceService {
    pub fn create_file(
        root: impl AsRef<Path>,
        command: WorkspaceCreateFileCommand,
    ) -> Result<WorkspacePathView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(&command.relative_path)?;
        if relative_path.is_empty() {
            return Err(AppCoreError::BadRequest("file path cannot be empty".to_string()));
        }

        let path = resolve_workspace_path_for_write(root, &relative_path)?;
        if path.exists() {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` already exists"
            )));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, []).map_err(|error| {
            AppCoreError::Internal(format!("failed to create file {}: {error}", path.display()))
        })?;

        Ok(WorkspacePathView { relative_path })
    }

    pub fn create_directory(
        root: impl AsRef<Path>,
        command: WorkspaceCreateDirectoryCommand,
    ) -> Result<WorkspacePathView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(&command.relative_path)?;
        if relative_path.is_empty() {
            return Err(AppCoreError::BadRequest("directory path cannot be empty".to_string()));
        }

        let path = resolve_workspace_path_for_write(root, &relative_path)?;
        if path.exists() {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` already exists"
            )));
        }
        fs::create_dir_all(&path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create directory {}: {error}",
                path.display()
            ))
        })?;

        Ok(WorkspacePathView { relative_path })
    }

    pub fn rename_path(
        root: impl AsRef<Path>,
        command: WorkspaceRenamePathCommand,
    ) -> Result<WorkspacePathView, AppCoreError> {
        let root = root.as_ref();
        let from_relative_path = normalize_relative_path(&command.from_relative_path)?;
        let to_relative_path = normalize_relative_path(&command.to_relative_path)?;
        if from_relative_path.is_empty() || to_relative_path.is_empty() {
            return Err(AppCoreError::BadRequest("workspace path cannot be empty".to_string()));
        }

        let from_path = resolve_workspace_path(root, &from_relative_path)?;
        let to_path = resolve_workspace_path_for_write(root, &to_relative_path)?;
        if to_path.exists() {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{to_relative_path}` already exists"
            )));
        }
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::rename(&from_path, &to_path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to rename {} to {}: {error}",
                from_path.display(),
                to_path.display()
            ))
        })?;

        Ok(WorkspacePathView { relative_path: to_relative_path })
    }

    pub fn delete_path(
        root: impl AsRef<Path>,
        command: WorkspaceDeletePathCommand,
    ) -> Result<WorkspacePathView, AppCoreError> {
        let root = root.as_ref();
        let relative_path = normalize_relative_path(&command.relative_path)?;
        if relative_path.is_empty() {
            return Err(AppCoreError::BadRequest("workspace path cannot be empty".to_string()));
        }

        let path = resolve_workspace_path(root, &relative_path)?;
        let metadata = fs::metadata(&path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to read file metadata {}: {error}",
                path.display()
            ))
        })?;
        if metadata.is_dir() {
            if command.recursive {
                fs::remove_dir_all(&path).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to delete directory {}: {error}",
                        path.display()
                    ))
                })?;
            } else {
                fs::remove_dir(&path).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to delete directory {}: {error}",
                        path.display()
                    ))
                })?;
            }
        } else {
            fs::remove_file(&path).map_err(|error| {
                AppCoreError::Internal(format!("failed to delete file {}: {error}", path.display()))
            })?;
        }

        Ok(WorkspacePathView { relative_path })
    }

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
        GitRepository::new(root.as_ref()).status().map_err(map_git_error)
    }

    pub fn git_stage(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        normalize_relative_path(path)?;
        GitRepository::new(root.as_ref()).stage(path).map_err(map_git_error)
    }

    pub fn git_unstage(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        normalize_relative_path(path)?;
        GitRepository::new(root.as_ref()).unstage(path).map_err(map_git_error)
    }

    pub fn git_discard(
        root: impl AsRef<Path>,
        path: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        normalize_relative_path(path)?;
        GitRepository::new(root.as_ref()).discard(path).map_err(map_git_error)
    }

    pub fn git_commit(
        root: impl AsRef<Path>,
        message: &str,
    ) -> Result<WorkspaceGitOperationView, AppCoreError> {
        let result = GitRepository::new(root.as_ref())
            .commit(message, GitCommitOptions::workspace_default())
            .map_err(map_git_error)?;
        Ok(WorkspaceGitOperationView { status: result.status })
    }

    pub fn git_diff(
        root: impl AsRef<Path>,
        path: &str,
        staged: bool,
    ) -> Result<WorkspaceGitDiffView, AppCoreError> {
        normalize_relative_path(path)?;
        GitRepository::new(root.as_ref()).path_diff(path, staged).map_err(map_git_error)
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

fn map_git_error(error: GitError) -> AppCoreError {
    match error {
        GitError::InvalidPath(message)
        | GitError::CommandFailed(message)
        | GitError::NotRepository(message) => AppCoreError::BadRequest(message),
        GitError::GitUnavailable => {
            AppCoreError::BadRequest("Git is not available on PATH".to_string())
        }
        GitError::RepositoryDiscovery(message) | GitError::RepositoryStatus(message) => {
            AppCoreError::Internal(message)
        }
        GitError::Io(error) => AppCoreError::Internal(error.to_string()),
    }
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

fn decode_limited_output(bytes: &[u8]) -> String {
    decode_limited_output_with_limit(bytes, MAX_CONSOLE_OUTPUT_BYTES)
}

fn decode_limited_output_with_limit(bytes: &[u8], limit: usize) -> String {
    if bytes.len() <= limit {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    let mut output = String::from_utf8_lossy(&bytes[..limit]).into_owned();
    output.push_str("\n[output truncated]\n");
    output
}

fn normalize_relative_path(raw: &str) -> Result<String, AppCoreError> {
    let normalized = slab_utils::path::normalize_relative_path_allow_empty(raw)
        .map_err(|_| AppCoreError::BadRequest(format!("workspace path `{raw}` is invalid")))?;
    if normalized.split('/').any(|segment| segment == ".slab") {
        return Err(AppCoreError::BadRequest(
            "workspace internals cannot be edited from the file tree".to_string(),
        ));
    }
    Ok(normalized)
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

fn resolve_workspace_path(root: &Path, relative_path: &str) -> Result<PathBuf, AppCoreError> {
    let canonical_root = root.canonicalize().map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to resolve workspace root {}: {error}",
            root.display()
        ))
    })?;
    let candidate = if relative_path.is_empty() {
        canonical_root.clone()
    } else {
        canonical_root.join(relative_path)
    };
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

    Ok(canonical_candidate)
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
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::normalize_relative_path;

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../outside.txt").is_err());
    }

    #[test]
    fn normalize_relative_path_rejects_workspace_internals() {
        assert!(normalize_relative_path(".slab/settings.json").is_err());
    }
}
