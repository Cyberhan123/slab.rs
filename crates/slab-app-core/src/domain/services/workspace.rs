use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slab_file::FileSystemError;
use slab_git::{GitCommitOptions, GitError, GitRepository};
use slab_utils::hash::{sha256_hex_bytes, verify_sha256_hex_expected};
use slab_utils::pty::spawn_pipe_process_no_stdin;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::domain::models::{
    WorkspaceConsoleOutput, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileEntry,
    WorkspaceFileKind, WorkspaceFileSearchView, WorkspaceGitDiffView, WorkspaceGitOperationView,
    WorkspaceGitStatusView, WorkspacePathMetadata, WorkspacePathView, WorkspaceRenamePathCommand,
    WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
use crate::error::AppCoreError;

use super::workspace_file_system::LocalExecutorFileSystem;

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONSOLE_COMMAND_BYTES: usize = 2_000;
const MAX_CONSOLE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 500;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_WRITE_FILE_BYTES: usize = 2 * 1024 * 1024;
const SLAB_DIR_NAME: &str = ".slab";
const IGNORED_DIR_NAMES: &[&str] = &[
    SLAB_DIR_NAME,
    ".git",
    ".hg",
    ".svn",
    ".idea",
    ".vscode",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".cache",
];

pub struct WorkspaceService;

impl WorkspaceService {
    pub fn read_directory(
        root: impl AsRef<Path>,
        relative_path: Option<&str>,
        include_ignored: bool,
    ) -> Result<WorkspaceDirectoryView, AppCoreError> {
        let relative_path = normalize_relative_path(relative_path.unwrap_or(""))?;
        let fs = LocalExecutorFileSystem::new(root.as_ref()).map_err(map_fs_error)?;
        let metadata = fs.metadata_sync(&relative_path).map_err(map_fs_error)?;
        if !metadata.is_directory {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` is not a directory"
            )));
        }

        let max_entries = if include_ignored { usize::MAX } else { MAX_DIRECTORY_ENTRIES };
        let mut entries = Vec::new();
        let mut truncated = false;
        for entry in fs.read_directory_sync(&relative_path).map_err(map_fs_error)? {
            if should_hide_entry(&entry.name, entry.metadata.is_directory, include_ignored) {
                continue;
            }
            if entries.len() >= max_entries {
                truncated = true;
                break;
            }

            entries.push(WorkspaceFileEntry {
                id: entry.path.clone(),
                name: entry.name,
                relative_path: entry.path,
                kind: if entry.metadata.is_directory {
                    WorkspaceFileKind::Directory
                } else {
                    WorkspaceFileKind::File
                },
                has_children: entry.metadata.is_directory,
                size_bytes: Some(if entry.metadata.is_file {
                    entry.metadata.size_bytes
                } else {
                    0
                }),
                modified_at: Some(entry.metadata.modified_at),
                created_at: Some(entry.metadata.created_at),
            });
        }

        entries.sort_by(|left, right| match (&left.kind, &right.kind) {
            (WorkspaceFileKind::Directory, WorkspaceFileKind::File) => std::cmp::Ordering::Less,
            (WorkspaceFileKind::File, WorkspaceFileKind::Directory) => std::cmp::Ordering::Greater,
            (WorkspaceFileKind::Directory, WorkspaceFileKind::Directory)
            | (WorkspaceFileKind::File, WorkspaceFileKind::File) => {
                left.name.to_lowercase().cmp(&right.name.to_lowercase())
            }
        });

        Ok(WorkspaceDirectoryView { relative_path, entries, truncated })
    }

    pub fn stat_path(
        root: impl AsRef<Path>,
        relative_path: &str,
    ) -> Result<WorkspacePathMetadata, AppCoreError> {
        let relative_path = normalize_relative_path(relative_path)?;
        let fs = LocalExecutorFileSystem::new(root.as_ref()).map_err(map_fs_error)?;
        let metadata = fs.metadata_sync(&relative_path).map_err(map_fs_error)?;
        let kind = if metadata.is_directory {
            WorkspaceFileKind::Directory
        } else if metadata.is_file {
            WorkspaceFileKind::File
        } else {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` is not a file or directory"
            )));
        };

        Ok(WorkspacePathMetadata {
            relative_path,
            kind,
            size_bytes: if metadata.is_file { metadata.size_bytes } else { 0 },
            modified_at: metadata.modified_at,
            created_at: metadata.created_at,
        })
    }

    pub fn read_file(
        root: impl AsRef<Path>,
        relative_path: &str,
    ) -> Result<WorkspaceFileContent, AppCoreError> {
        let relative_path = normalize_relative_path(relative_path)?;
        let fs = LocalExecutorFileSystem::new(root.as_ref()).map_err(map_fs_error)?;
        let metadata = fs.metadata_sync(&relative_path).map_err(map_fs_error)?;
        if !metadata.is_file {
            return Err(AppCoreError::BadRequest(format!(
                "workspace path `{relative_path}` is not a file"
            )));
        }
        if metadata.size_bytes > MAX_FILE_BYTES {
            return Err(AppCoreError::BadRequest(format!(
                "file is too large to preview ({} bytes, limit {} bytes)",
                metadata.size_bytes, MAX_FILE_BYTES
            )));
        }

        let bytes = fs.read_file_bytes(&relative_path).map_err(map_fs_error)?;
        if bytes.contains(&0) {
            return Err(AppCoreError::BadRequest("binary files cannot be previewed".to_string()));
        }
        let content = String::from_utf8(bytes)
            .map_err(|_| AppCoreError::BadRequest("file is not valid UTF-8".to_string()))?;
        let name = Path::new(&relative_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(relative_path.as_str())
            .to_owned();
        let content_hash = content_hash(content.as_bytes());

        Ok(WorkspaceFileContent {
            relative_path,
            name,
            content,
            size_bytes: metadata.size_bytes,
            content_hash,
        })
    }

    pub fn search_files(
        root: impl AsRef<Path>,
        query: &str,
    ) -> Result<WorkspaceFileSearchView, AppCoreError> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Ok(WorkspaceFileSearchView { query, entries: Vec::new(), truncated: false });
        }

        let mut options = slab_file::search::FileSearchOptions::new(root.as_ref(), &query);
        options.limit = MAX_SEARCH_RESULTS;
        options.include_dirs = false;
        options.include_hidden = false;
        options.extra_ignore_names =
            IGNORED_DIR_NAMES.iter().map(|name| (*name).to_string()).collect();
        let snapshot = slab_file::search::run(options)
            .map_err(|error| AppCoreError::Internal(error.to_string()))?;
        let entries = snapshot
            .matches
            .into_iter()
            .map(|matched| WorkspaceFileEntry {
                id: matched.relative_path.clone(),
                name: matched.name,
                relative_path: matched.relative_path,
                kind: WorkspaceFileKind::File,
                has_children: false,
                size_bytes: None,
                modified_at: None,
                created_at: None,
            })
            .collect();

        Ok(WorkspaceFileSearchView { query, entries, truncated: snapshot.truncated })
    }

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
                if verify_sha256_hex_expected(&current_hash, expected_hash).is_err() {
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

        let (program, args) = shell_command(command);
        let env: HashMap<String, String> = std::env::vars().collect();
        let spawned = spawn_pipe_process_no_stdin(&program, &args, root.as_ref(), &env, &None)
            .await
            .map_err(|error| {
                AppCoreError::Internal(format!("failed to run workspace command: {error}"))
            })?;
        let session = spawned.session;
        let stdout_task = tokio::spawn(collect_limited_output(spawned.stdout_rx));
        let stderr_task = tokio::spawn(collect_limited_output(spawned.stderr_rx));

        let console_output = async {
            let exit_code = spawned.exit_rx.await.unwrap_or(-1);
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            (exit_code, stdout, stderr)
        };
        let (exit_code, stdout, stderr) = match timeout(CONSOLE_TIMEOUT, console_output).await {
            Ok(output) => output,
            Err(_) => {
                session.terminate();
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
            exit_code: (exit_code >= 0).then_some(exit_code),
            stdout: decode_limited_output(&stdout),
            stderr: decode_limited_output(&stderr),
            timed_out: false,
        })
    }
}

fn map_fs_error(error: FileSystemError) -> AppCoreError {
    match error {
        FileSystemError::AbsolutePath(message)
        | FileSystemError::PathEscapesRoot(message)
        | FileSystemError::InvalidPath(message) => AppCoreError::BadRequest(message),
        FileSystemError::Root(error) | FileSystemError::Io(error) => {
            AppCoreError::Internal(error.to_string())
        }
        FileSystemError::InvalidPatch(message) => AppCoreError::BadRequest(message),
        FileSystemError::PatchMismatch { path, line } => {
            AppCoreError::BadRequest(format!("patch does not apply to `{path}` at line {line}"))
        }
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
fn shell_command(command: &str) -> (String, Vec<String>) {
    (
        "powershell.exe".to_owned(),
        vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-Command".to_owned(),
            command.to_owned(),
        ],
    )
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> (String, Vec<String>) {
    ("sh".to_owned(), vec!["-lc".to_owned(), command.to_owned()])
}

fn decode_limited_output(bytes: &[u8]) -> String {
    decode_limited_output_with_limit(bytes, MAX_CONSOLE_OUTPUT_BYTES)
}

async fn collect_limited_output(mut rx: mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    let mut output = Vec::new();
    let limit = MAX_CONSOLE_OUTPUT_BYTES + 1;
    while let Some(chunk) = rx.recv().await {
        if output.len() < limit {
            let remaining = limit - output.len();
            output.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
    }
    output
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
    if normalized.split('/').any(|segment| segment == SLAB_DIR_NAME) {
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

fn should_hide_entry(name: &str, is_directory: bool, include_ignored: bool) -> bool {
    !include_ignored
        && is_directory
        && IGNORED_DIR_NAMES.iter().any(|ignored| ignored.eq_ignore_ascii_case(name))
}

fn content_hash(bytes: &[u8]) -> String {
    sha256_hex_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{WorkspaceService, normalize_relative_path};

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../outside.txt").is_err());
    }

    #[test]
    fn normalize_relative_path_rejects_workspace_internals() {
        assert!(normalize_relative_path(".slab/settings.json").is_err());
    }

    #[test]
    fn read_directory_hides_ignored_workspace_directories() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("src")).expect("src");
        fs::create_dir_all(root.path().join("node_modules")).expect("node modules");

        let view = WorkspaceService::read_directory(root.path(), None, false).expect("directory");

        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].relative_path, "src");
    }

    #[test]
    fn search_files_uses_gitignore() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("src")).expect("src");
        fs::write(root.path().join(".gitignore"), "ignored.rs\n").expect("gitignore");
        fs::write(root.path().join("src").join("workspace_search.rs"), "").expect("source");
        fs::write(root.path().join("src").join("ignored.rs"), "").expect("ignored");

        let view = WorkspaceService::search_files(root.path(), "wss").expect("search");

        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].relative_path, "src/workspace_search.rs");
    }
}
