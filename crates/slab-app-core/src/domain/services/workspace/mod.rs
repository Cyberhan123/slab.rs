mod file_system;
mod lsp;

pub use lsp::WorkspaceLspService;
pub(crate) use lsp::workspace_root_from_config;

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Map, Value};
use slab_config::SettingsDocument;
use slab_file::{DirectoryEntry, FileSystemError};
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
use crate::schemas::workspace::{
    WorkspaceConfigResponse, WorkspacePluginConfig, WorkspacePluginPreferenceUpdate,
};

use self::file_system::LocalExecutorFileSystem;

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_CONSOLE_COMMAND_BYTES: usize = 2_000;
const MAX_CONSOLE_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 500;
const MAX_DIRECTORY_DEPTH: u8 = 5;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_LINE_MATCHES_PER_FILE: usize = 20;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_WRITE_FILE_BYTES: usize = 2 * 1024 * 1024;
const SLAB_DIR_NAME: &str = ".slab";
const SETTINGS_FILE: &str = "settings.json";
const LEGACY_SETTINGS_FILE: &str = "workspace.json";
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

    pub fn ensure_workspace_settings(root: impl AsRef<Path>) -> Result<PathBuf, AppCoreError> {
        let root = root.as_ref();
        let slab_dir = root.join(SLAB_DIR_NAME);
        fs::create_dir_all(&slab_dir).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create workspace settings directory {}: {error}",
                slab_dir.display()
            ))
        })?;
        let settings_path = slab_dir.join(SETTINGS_FILE);
        migrate_legacy_workspace_config(&slab_dir.join(LEGACY_SETTINGS_FILE), &settings_path)?;
        ensure_workspace_settings_file(&settings_path)?;
        Ok(settings_path)
    }

    pub fn workspace_config(
        root: impl AsRef<Path>,
    ) -> Result<WorkspaceConfigResponse, AppCoreError> {
        load_workspace_config(&workspace_settings_path(root.as_ref()))
    }

    pub fn update_workspace_plugin_preference(
        root: impl AsRef<Path>,
        plugin_id: &str,
        update: WorkspacePluginPreferenceUpdate,
    ) -> Result<WorkspaceConfigResponse, AppCoreError> {
        validate_plugin_id(plugin_id)?;
        let settings_path = workspace_settings_path(root.as_ref());
        write_workspace_plugin_preference(&settings_path, plugin_id, &update)?;
        load_workspace_config(&settings_path)
    }

    pub fn read_directory(
        root: impl AsRef<Path>,
        relative_path: Option<&str>,
        include_ignored: bool,
        depth: u8,
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
        // `depth` is the number of directory levels to expand below the requested
        // directory: 1 (the default) lists only its direct children (legacy
        // behavior); higher values flatten nested entries into a single response
        // so the file-tree overlay can pre-load without one request per folder.
        let depth = depth.clamp(1, MAX_DIRECTORY_DEPTH);
        let mut entries = Vec::new();
        let mut truncated = false;
        read_directory_recursive(
            &fs,
            &relative_path,
            depth,
            include_ignored,
            max_entries,
            &mut entries,
            &mut truncated,
        )?;

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
        Self::run_console_command_with_timeout(root.as_ref(), command, CONSOLE_TIMEOUT).await
    }

    async fn run_console_command_with_timeout(
        root: &Path,
        command: &str,
        timeout_duration: Duration,
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
        let spawned =
            spawn_pipe_process_no_stdin(&program, &args, root, &env, &None).await.map_err(
                |error| AppCoreError::Internal(format!("failed to run workspace command: {error}")),
            )?;
        let session = spawned.session;
        let stdout_task = tokio::spawn(collect_limited_output(spawned.stdout_rx));
        let stderr_task = tokio::spawn(collect_limited_output(spawned.stderr_rx));

        let console_output = async {
            let exit_code = spawned.exit_rx.await.unwrap_or(-1);
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            (exit_code, stdout, stderr)
        };
        let (exit_code, stdout, stderr) = match timeout(timeout_duration, console_output).await {
            Ok(output) => output,
            Err(_) => {
                session.terminate();
                return Ok(WorkspaceConsoleOutput {
                    command: command.to_string(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!(
                        "Command timed out after {} seconds.",
                        timeout_duration.as_secs()
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

/// Recursively flattens directory entries up to `remaining_depth` levels below
/// the requested directory. Directories are descended into while the depth
/// budget remains; symlinked directories are listed but not traversed (bounding
/// cost and avoiding cycles). The same ignore filter and entry cap apply at
/// every level, so the result is consistent with a single-level listing.
fn read_directory_recursive(
    fs: &LocalExecutorFileSystem,
    relative_path: &str,
    remaining_depth: u8,
    include_ignored: bool,
    max_entries: usize,
    entries: &mut Vec<WorkspaceFileEntry>,
    truncated: &mut bool,
) -> Result<(), AppCoreError> {
    if *truncated {
        return Ok(());
    }
    for entry in fs.read_directory_sync(relative_path).map_err(map_fs_error)? {
        if should_hide_entry(&entry.name, entry.metadata.is_directory, include_ignored) {
            continue;
        }
        if entries.len() >= max_entries {
            *truncated = true;
            break;
        }

        let descend = entry.metadata.is_directory
            && !entry.metadata.is_symlink
            && remaining_depth > 1
            && !*truncated;
        entries.push(directory_entry_to_file_entry(&entry));

        if descend {
            read_directory_recursive(
                fs,
                &entry.path,
                remaining_depth - 1,
                include_ignored,
                max_entries,
                entries,
                truncated,
            )?;
        }
        if *truncated {
            break;
        }
    }
    Ok(())
}

fn directory_entry_to_file_entry(entry: &DirectoryEntry) -> WorkspaceFileEntry {
    WorkspaceFileEntry {
        id: entry.path.clone(),
        name: entry.name.clone(),
        relative_path: entry.path.clone(),
        kind: if entry.metadata.is_directory {
            WorkspaceFileKind::Directory
        } else {
            WorkspaceFileKind::File
        },
        has_children: entry.metadata.is_directory,
        size_bytes: Some(if entry.metadata.is_file { entry.metadata.size_bytes } else { 0 }),
        modified_at: Some(entry.metadata.modified_at),
        created_at: Some(entry.metadata.created_at),
    }
}

fn map_fs_error(error: FileSystemError) -> AppCoreError {
    match error {
        FileSystemError::AbsolutePath(message)
        | FileSystemError::PathEscapesRoot(message)
        | FileSystemError::InvalidPath(message)
        | FileSystemError::PermissionDenied(message) => AppCoreError::BadRequest(message),
        FileSystemError::Root(error) | FileSystemError::Io(error) => {
            // A missing path (e.g. an optional `.vscode/settings.json` probe, or a
            // directory that was removed) is a client error, not a server fault.
            if error.kind() == std::io::ErrorKind::NotFound {
                AppCoreError::NotFound(error.to_string())
            } else {
                AppCoreError::Internal(error.to_string())
            }
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

fn workspace_settings_path(root: &Path) -> PathBuf {
    root.join(SLAB_DIR_NAME).join(SETTINGS_FILE)
}

fn load_workspace_config(path: &Path) -> Result<WorkspaceConfigResponse, AppCoreError> {
    let settings = load_settings_overlay(path)?;
    workspace_config_from_settings_overlay(&settings)
}

fn load_settings_overlay(path: &Path) -> Result<Value, AppCoreError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = fs::read_to_string(path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read workspace settings {}: {error}",
            path.display()
        ))
    })?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to parse workspace settings {}: {error}",
            path.display()
        ))
    })?;
    if !value.is_object() {
        return Err(AppCoreError::BadRequest(format!(
            "workspace settings {} must contain a JSON object",
            path.display()
        )));
    }
    Ok(value)
}

fn workspace_config_from_settings_overlay(
    settings: &Value,
) -> Result<WorkspaceConfigResponse, AppCoreError> {
    let schema_version = settings
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or_else(|| SettingsDocument::default().schema_version);
    let mut plugins = BTreeMap::new();

    if let Some(plugin_map) = settings
        .get("workspace")
        .and_then(|workspace| workspace.get("plugins"))
        .and_then(Value::as_object)
    {
        for (plugin_id, value) in plugin_map {
            let config: WorkspacePluginConfig =
                serde_json::from_value(value.clone()).map_err(|error| {
                    AppCoreError::BadRequest(format!(
                        "workspace plugin preference `{plugin_id}` has invalid shape: {error}"
                    ))
                })?;
            if config.enabled.is_some() {
                plugins.insert(plugin_id.clone(), config);
            }
        }
    }

    Ok(WorkspaceConfigResponse { schema_version, plugins })
}

fn write_workspace_plugin_preference(
    settings_path: &Path,
    plugin_id: &str,
    update: &WorkspacePluginPreferenceUpdate,
) -> Result<(), AppCoreError> {
    let mut settings = load_settings_overlay(settings_path)?;
    if update.enabled == Some(false) {
        set_workspace_plugin_enabled(&mut settings, plugin_id, false);
    } else {
        remove_workspace_plugin_preference(&mut settings, plugin_id);
    }
    prune_empty_objects(&mut settings);
    write_json_file(settings_path, &settings)
}

fn ensure_workspace_settings_file(path: &Path) -> Result<(), AppCoreError> {
    if !path.exists() {
        write_json_file(path, &Value::Object(Map::new()))?;
    }
    Ok(())
}

fn migrate_legacy_workspace_config(
    legacy_path: &Path,
    settings_path: &Path,
) -> Result<(), AppCoreError> {
    if !legacy_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(legacy_path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to read legacy workspace config {}: {error}",
            legacy_path.display()
        ))
    })?;
    let legacy: WorkspaceConfigResponse = serde_json::from_str(&raw).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to parse legacy workspace config {}: {error}",
            legacy_path.display()
        ))
    })?;
    if legacy.plugins.is_empty() {
        return Ok(());
    }

    let mut settings = load_settings_overlay(settings_path)?;
    let mut changed = false;
    for (plugin_id, plugin_config) in legacy.plugins {
        let Some(enabled) = plugin_config.enabled else {
            continue;
        };
        if !workspace_plugin_enabled_exists(&settings, &plugin_id) {
            set_workspace_plugin_enabled(&mut settings, &plugin_id, enabled);
            changed = true;
        }
    }

    if changed {
        prune_empty_objects(&mut settings);
        write_json_file(settings_path, &settings)?;
    }

    Ok(())
}

fn workspace_plugin_enabled_exists(settings: &Value, plugin_id: &str) -> bool {
    settings
        .get("workspace")
        .and_then(|workspace| workspace.get("plugins"))
        .and_then(|plugins| plugins.get(plugin_id))
        .and_then(|plugin| plugin.get("enabled"))
        .is_some()
}

fn set_workspace_plugin_enabled(settings: &mut Value, plugin_id: &str, enabled: bool) {
    let root = settings.as_object_mut().expect("settings overlay object checked");
    let workspace = child_object(root, "workspace");
    let plugins = child_object(workspace, "plugins");
    let plugin = child_object(plugins, plugin_id);
    plugin.insert("enabled".to_owned(), Value::Bool(enabled));
}

fn remove_workspace_plugin_preference(settings: &mut Value, plugin_id: &str) {
    let Some(plugins) = settings
        .get_mut("workspace")
        .and_then(|workspace| workspace.get_mut("plugins"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    plugins.remove(plugin_id);
}

fn child_object<'a>(parent: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let value = parent.entry(key.to_owned()).or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("object inserted")
}

fn prune_empty_objects(value: &mut Value) {
    let Value::Object(object) = value else {
        return;
    };

    let keys = object.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        if let Some(child) = object.get_mut(&key) {
            prune_empty_objects(child);
            if child.as_object().is_some_and(Map::is_empty) {
                object.remove(&key);
            }
        }
    }
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), AppCoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|error| {
        AppCoreError::Internal(format!("failed to serialize JSON for {}: {error}", path.display()))
    })?;
    fs::write(path, format!("{content}\n")).map_err(|error| {
        AppCoreError::Internal(format!("failed to write JSON file {}: {error}", path.display()))
    })
}

fn validate_plugin_id(plugin_id: &str) -> Result<(), AppCoreError> {
    let valid = (2..=64).contains(&plugin_id.len())
        && plugin_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        });
    valid
        .then_some(())
        .ok_or_else(|| AppCoreError::BadRequest(format!("invalid plugin id `{plugin_id}`")))
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
    use std::time::Duration;

    use super::{
        AppCoreError, MAX_CONSOLE_OUTPUT_BYTES, WorkspaceService, normalize_relative_path,
    };

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

        let view =
            WorkspaceService::read_directory(root.path(), None, false, 1).expect("directory");

        assert_eq!(view.entries.len(), 1);
        assert_eq!(view.entries[0].relative_path, "src");
    }

    #[test]
    fn read_directory_maps_missing_directory_to_not_found() {
        let root = tempfile::tempdir().expect("tempdir");
        let error = WorkspaceService::read_directory(root.path(), Some("does/not/exist"), false, 1)
            .expect_err("missing directory");
        assert!(matches!(error, AppCoreError::NotFound(_)));
    }

    #[test]
    fn stat_path_maps_missing_file_to_not_found() {
        let root = tempfile::tempdir().expect("tempdir");
        let error = WorkspaceService::stat_path(root.path(), ".vscode/settings.json")
            .expect_err("missing file");
        assert!(matches!(error, AppCoreError::NotFound(_)));
    }

    #[test]
    fn read_directory_depth_flattens_nested_entries() {
        let root = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(root.path().join("src/components")).expect("nested dirs");
        fs::write(root.path().join("src/main.rs"), "fn main() {}").expect("seed file");
        fs::write(root.path().join("src/components/Button.tsx"), "x").expect("seed nested");

        let shallow =
            WorkspaceService::read_directory(root.path(), None, false, 1).expect("shallow");
        let shallow_paths: Vec<String> =
            shallow.entries.iter().map(|entry| entry.relative_path.clone()).collect();
        assert!(shallow_paths.contains(&"src".to_owned()));
        assert!(!shallow_paths.contains(&"src/main.rs".to_owned()));

        // depth=2 flattens levels 1 and 2 (src, plus src's direct children), but
        // does not descend into level-2 directories (src/components/...).
        let deep = WorkspaceService::read_directory(root.path(), None, false, 2).expect("deep");
        let deep_paths: Vec<String> =
            deep.entries.iter().map(|entry| entry.relative_path.clone()).collect();
        assert!(deep_paths.contains(&"src/main.rs".to_owned()));
        assert!(deep_paths.contains(&"src/components".to_owned()));
        assert!(!deep_paths.contains(&"src/components/Button.tsx".to_owned()));

        let deeper = WorkspaceService::read_directory(root.path(), None, false, 3).expect("deeper");
        let deeper_paths: Vec<String> =
            deeper.entries.iter().map(|entry| entry.relative_path.clone()).collect();
        assert!(deeper_paths.contains(&"src/components/Button.tsx".to_owned()));
        // Ignored directories are still hidden at nested levels.
        assert!(
            !deeper_paths
                .iter()
                .any(|path| path.split('/').any(|segment| segment == "node_modules"))
        );
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

    #[test]
    fn ensure_workspace_settings_creates_settings_file() {
        let root = tempfile::tempdir().expect("tempdir");

        let settings_path =
            WorkspaceService::ensure_workspace_settings(root.path()).expect("workspace settings");

        assert_eq!(settings_path, root.path().join(".slab").join("settings.json"));
        assert!(settings_path.exists());
    }

    #[test]
    fn workspace_config_reads_plugin_preferences_from_settings_overlay() {
        let root = tempfile::tempdir().expect("tempdir");
        let settings_path =
            WorkspaceService::ensure_workspace_settings(root.path()).expect("workspace settings");
        fs::write(
            &settings_path,
            r#"{
  "workspace": {
    "plugins": {
      "video-subtitle_translator": { "enabled": false },
      "empty": {}
    }
  }
}
"#,
        )
        .expect("write settings");

        let config = WorkspaceService::workspace_config(root.path()).expect("workspace config");

        assert_eq!(config.plugins.len(), 1);
        assert_eq!(
            config.plugins.get("video-subtitle_translator").and_then(|config| config.enabled),
            Some(false)
        );
    }

    #[test]
    fn update_workspace_plugin_preference_writes_and_removes_disabled_override() {
        let root = tempfile::tempdir().expect("tempdir");
        WorkspaceService::ensure_workspace_settings(root.path()).expect("workspace settings");

        let disabled = WorkspaceService::update_workspace_plugin_preference(
            root.path(),
            "video-subtitle_translator",
            crate::schemas::workspace::WorkspacePluginPreferenceUpdate { enabled: Some(false) },
        )
        .expect("disable plugin");
        assert_eq!(
            disabled.plugins.get("video-subtitle_translator").and_then(|config| config.enabled),
            Some(false)
        );

        let restored = WorkspaceService::update_workspace_plugin_preference(
            root.path(),
            "video-subtitle_translator",
            crate::schemas::workspace::WorkspacePluginPreferenceUpdate { enabled: Some(true) },
        )
        .expect("restore plugin");
        assert!(!restored.plugins.contains_key("video-subtitle_translator"));
        let raw =
            fs::read_to_string(root.path().join(".slab").join("settings.json")).expect("settings");
        assert!(!raw.contains("video-subtitle_translator"));
    }

    #[test]
    fn update_workspace_plugin_preference_rejects_invalid_plugin_ids() {
        let root = tempfile::tempdir().expect("tempdir");
        WorkspaceService::ensure_workspace_settings(root.path()).expect("workspace settings");

        let error = WorkspaceService::update_workspace_plugin_preference(
            root.path(),
            "Plugin",
            crate::schemas::workspace::WorkspacePluginPreferenceUpdate { enabled: Some(false) },
        )
        .expect_err("invalid plugin id");

        assert!(error.to_string().contains("invalid plugin id"));
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

    #[tokio::test]
    async fn run_console_command_truncates_long_stdout() {
        let root = tempfile::tempdir().expect("tempdir");
        let command = if cfg!(windows) {
            "'x' * 70000".to_string()
        } else {
            "i=0; while [ \"$i\" -lt 70000 ]; do printf x; i=$((i + 1)); done".to_string()
        };

        let output = WorkspaceService::run_console_command(root.path(), &command)
            .await
            .expect("console output");

        assert_eq!(output.exit_code, Some(0));
        assert!(!output.timed_out);
        assert!(output.stdout.contains("[output truncated]"));
        assert!(output.stdout.len() <= MAX_CONSOLE_OUTPUT_BYTES + "\n[output truncated]\n".len());
    }

    #[tokio::test]
    async fn run_console_command_reports_timeout_without_waiting_for_default_timeout() {
        let root = tempfile::tempdir().expect("tempdir");
        let command = if cfg!(windows) { "Start-Sleep -Seconds 2" } else { "sleep 2" }.to_string();

        let output = WorkspaceService::run_console_command_with_timeout(
            root.path(),
            &command,
            Duration::from_millis(100),
        )
        .await
        .expect("console output");

        assert_eq!(output.exit_code, None);
        assert!(output.timed_out);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.contains("Command timed out after"));
    }
}
