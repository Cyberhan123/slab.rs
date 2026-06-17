mod file_system;
mod lsp;

pub use lsp::WorkspaceLspService;
pub(crate) use lsp::workspace_root_from_config;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use slab_file::FileSystemError;
use slab_git::{GitCommitOptions, GitError, GitRepository};
use slab_utils::hash::{sha256_hex_bytes, verify_sha256_hex_expected};
use slab_utils::pty::spawn_pipe_process_no_stdin;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::context::AppConfig;
use crate::domain::models::{
    WorkspaceConsoleOutput, WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand,
    WorkspaceDeletePathCommand, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileEntry,
    WorkspaceFileKind, WorkspaceFileSearchView, WorkspaceGitDiffView, WorkspaceGitOperationView,
    WorkspaceGitStatusView, WorkspacePathMetadata, WorkspacePathView, WorkspaceRenamePathCommand,
    WorkspaceTextSearchFileMatch, WorkspaceTextSearchLineMatch, WorkspaceTextSearchView,
    WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
use crate::error::AppCoreError;

use self::file_system::LocalExecutorFileSystem;

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONSOLE_COMMAND_BYTES: usize = 2_000;
const MAX_CONSOLE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 500;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_LINE_MATCHES_PER_FILE: usize = 20;
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
    pub fn workspace_root_from_config(config: &AppConfig) -> Option<PathBuf> {
        lsp::workspace_root_from_config(config)
    }

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

    pub fn search_text(
        root: impl AsRef<Path>,
        query: &str,
    ) -> Result<WorkspaceTextSearchView, AppCoreError> {
        let query = query.trim().to_owned();
        if query.is_empty() {
            return Ok(WorkspaceTextSearchView { query, matches: Vec::new(), truncated: false });
        }

        let root = root.as_ref();
        let search_query = query.to_ascii_lowercase();
        let mut matches = Vec::new();
        let mut truncated = false;
        search_text_directory(root, "", &search_query, &mut matches, &mut truncated)?;

        Ok(WorkspaceTextSearchView { query, matches, truncated })
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
            stdout: slab_utils::string::decode_truncated_prefix(
                &stdout,
                MAX_CONSOLE_OUTPUT_BYTES,
                "\n[output truncated]\n",
            ),
            stderr: slab_utils::string::decode_truncated_prefix(
                &stderr,
                MAX_CONSOLE_OUTPUT_BYTES,
                "\n[output truncated]\n",
            ),
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
        let canonical_parent =
            slab_utils::fs::existing_ancestor(parent).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => {
                    AppCoreError::BadRequest("workspace path has no existing parent".to_string())
                }
                _ => AppCoreError::Internal(format!(
                    "failed to resolve workspace parent {}: {error}",
                    parent.display()
                )),
            })?;
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

fn should_hide_entry(name: &str, is_directory: bool, include_ignored: bool) -> bool {
    !include_ignored
        && is_directory
        && IGNORED_DIR_NAMES.iter().any(|ignored| ignored.eq_ignore_ascii_case(name))
}

fn content_hash(bytes: &[u8]) -> String {
    sha256_hex_bytes(bytes)
}

fn search_text_directory(
    directory: &Path,
    relative_path: &str,
    query: &str,
    matches: &mut Vec<WorkspaceTextSearchFileMatch>,
    truncated: &mut bool,
) -> Result<(), AppCoreError> {
    if matches.len() >= MAX_SEARCH_RESULTS {
        *truncated = true;
        return Ok(());
    }

    for entry in fs::read_dir(directory).map_err(|error| {
        AppCoreError::Internal(format!("failed to read directory {}: {error}", directory.display()))
    })? {
        if matches.len() >= MAX_SEARCH_RESULTS {
            *truncated = true;
            break;
        }

        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!("failed to read directory entry: {error}"))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to read file type {}: {error}",
                entry.path().display()
            ))
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_hide_entry(&name, file_type.is_dir(), false) {
            continue;
        }

        let entry_relative_path = join_relative_path(relative_path, &name);
        if file_type.is_dir() {
            search_text_directory(&entry.path(), &entry_relative_path, query, matches, truncated)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        if let Some(file_match) =
            search_text_file(&entry.path(), &entry_relative_path, &name, query, truncated)?
        {
            matches.push(file_match);
            if matches.len() >= MAX_SEARCH_RESULTS {
                *truncated = true;
                break;
            }
        }
    }

    Ok(())
}

fn search_text_file(
    path: &Path,
    relative_path: &str,
    name: &str,
    query: &str,
    truncated: &mut bool,
) -> Result<Option<WorkspaceTextSearchFileMatch>, AppCoreError> {
    let Some(content) = read_searchable_file(path)? else {
        return Ok(None);
    };

    let mut line_matches = Vec::new();
    for (index, line) in content.lines().enumerate() {
        let lower_line = line.to_ascii_lowercase();
        let Some(match_byte_start) = lower_line.find(query) else {
            continue;
        };
        let match_byte_end = match_byte_start + query.len();
        line_matches.push(WorkspaceTextSearchLineMatch {
            line_number: index + 1,
            line_text: line.to_owned(),
            match_start: line[..match_byte_start].chars().count(),
            match_end: line[..match_byte_end].chars().count(),
        });

        if line_matches.len() >= MAX_LINE_MATCHES_PER_FILE {
            *truncated = true;
            break;
        }
    }

    if line_matches.is_empty() {
        return Ok(None);
    }

    Ok(Some(WorkspaceTextSearchFileMatch {
        relative_path: relative_path.to_owned(),
        name: name.to_owned(),
        line_matches,
    }))
}

fn read_searchable_file(path: &Path) -> Result<Option<String>, AppCoreError> {
    let metadata = fs::metadata(path).map_err(|error| {
        AppCoreError::Internal(format!("failed to read file metadata {}: {error}", path.display()))
    })?;
    if metadata.len() > MAX_FILE_BYTES {
        return Ok(None);
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to open file {}: {error}", path.display()))
        })?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to read file {}: {error}", path.display()))
        })?;
    if bytes.contains(&0) {
        return Ok(None);
    }

    Ok(String::from_utf8(bytes).ok())
}

fn join_relative_path(parent: &str, child: &str) -> String {
    if parent.is_empty() { child.to_owned() } else { format!("{parent}/{child}") }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

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

    #[test]
    fn search_text_skips_binary_and_ignored_directories() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("src")).expect("src");
        fs::create_dir_all(root.path().join("node_modules")).expect("node modules");
        fs::write(root.path().join("src").join("main.rs"), "fn main() {\n  let Value = 1;\n}\n")
            .expect("write source");
        fs::write(root.path().join("node_modules").join("ignored.rs"), "let Value = 2;\n")
            .expect("write ignored");
        fs::write(root.path().join("binary.bin"), b"Value\0").expect("write binary");

        let response = WorkspaceService::search_text(root.path(), "value").expect("search text");

        assert!(!response.truncated);
        assert_eq!(response.matches.len(), 1);
        assert_eq!(response.matches[0].relative_path, "src/main.rs");
        assert_eq!(response.matches[0].line_matches[0].line_number, 2);
        assert_eq!(response.matches[0].line_matches[0].match_start, 6);
        assert_eq!(response.matches[0].line_matches[0].match_end, 11);
    }

    #[tokio::test]
    async fn run_console_command_uses_workspace_root_as_cwd() {
        let root = tempfile::tempdir().expect("tempdir");

        let command =
            if cfg!(windows) { "(Get-Location).Path".to_string() } else { "pwd".to_string() };
        let output = WorkspaceService::run_console_command(root.path(), &command)
            .await
            .expect("console output");

        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);

        let reported_cwd = PathBuf::from(output.stdout.trim());
        let canonical_reported = reported_cwd.canonicalize().expect("reported cwd");
        let canonical_root = root.path().canonicalize().expect("canonical root");
        assert_eq!(canonical_reported, canonical_root);
    }

    #[tokio::test]
    async fn run_console_command_supports_quoted_relative_paths() {
        let root = tempfile::tempdir().expect("tempdir");
        let relative_path = PathBuf::from("nested dir").join("file with spaces.txt");
        let file_path = root.path().join(&relative_path);
        fs::create_dir_all(file_path.parent().expect("parent")).expect("create nested dir");
        fs::write(&file_path, "quoted path content").expect("write test file");

        let command = if cfg!(windows) {
            "Get-Content -Raw '.\\nested dir\\file with spaces.txt'".to_string()
        } else {
            "cat './nested dir/file with spaces.txt'".to_string()
        };
        let output = WorkspaceService::run_console_command(root.path(), &command)
            .await
            .expect("console output");

        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout.trim_end_matches(['\r', '\n']), "quoted path content");
        assert!(output.stderr.is_empty());
        assert!(!output.timed_out);
    }

    #[tokio::test]
    async fn run_console_command_writes_relative_paths_inside_workspace_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let relative_path = "created by command.txt";
        let expected_content = "created from workspace command";

        let command = if cfg!(windows) {
            format!("Set-Content -NoNewline -Path '.\\{relative_path}' -Value '{expected_content}'")
        } else {
            format!("printf '{expected_content}' > './{relative_path}'")
        };
        let output = WorkspaceService::run_console_command(root.path(), &command)
            .await
            .expect("console output");

        assert_eq!(output.exit_code, Some(0));
        assert!(output.stderr.is_empty());
        assert!(!output.timed_out);
        assert_eq!(
            fs::read_to_string(root.path().join(relative_path)).expect("created file"),
            expected_content
        );
    }
}
