use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use dirs_next::config_dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use slab_app_core::domain::models::{
    WorkspaceConsoleOutput, WorkspaceGitCommitCommand, WorkspaceGitOperationView,
    WorkspaceGitPathCommand, WorkspaceGitStatusView, WorkspaceWriteFileCommand,
    WorkspaceWriteFileView,
};
use slab_app_core::domain::services::WorkspaceService;
use slab_types::settings::SettingsDocument;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::plugins;
use crate::setup::{ServerSidecarConfig, restart_server_sidecar};

const SLAB_DIR_NAME: &str = ".slab";
const WORKSPACE_CONFIG_FILE: &str = "workspace.json";
const SETTINGS_FILE: &str = "settings.json";
const DATABASE_FILE: &str = "slab.db";
const MAX_RECENT_WORKSPACES: usize = 10;
const MAX_DIRECTORY_ENTRIES: usize = 500;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
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

#[derive(Clone, Debug)]
pub struct WorkspaceBootstrap {
    pub sidecar_config: ServerSidecarConfig,
}

#[derive(Debug)]
pub struct WorkspaceState {
    recent_store_path: PathBuf,
    inner: RwLock<WorkspaceRuntimeState>,
}

#[derive(Debug, Default)]
struct WorkspaceRuntimeState {
    current: Option<WorkspaceInfo>,
    recent: Vec<RecentWorkspace>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub root_path: String,
    pub name: String,
    pub slab_dir: String,
    pub settings_path: String,
    pub workspace_config_path: String,
    pub database_path: String,
    pub model_config_dir: String,
    pub session_state_dir: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentWorkspace {
    pub root_path: String,
    pub name: String,
    pub last_opened_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStateResponse {
    pub current: Option<WorkspaceInfo>,
    pub recent: Vec<RecentWorkspace>,
    pub config: Option<WorkspaceConfig>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDirectoryResponse {
    pub relative_path: String,
    pub entries: Vec<WorkspaceFileEntry>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileEntry {
    pub id: String,
    pub name: String,
    pub relative_path: String,
    pub kind: WorkspaceFileKind,
    pub has_children: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceFileKind {
    Directory,
    File,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileContent {
    pub relative_path: String,
    pub name: String,
    pub content: String,
    pub size_bytes: u64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub schema_version: u32,
    #[serde(default)]
    pub plugins: BTreeMap<String, WorkspacePluginConfig>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePluginConfig {
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePluginPreferenceUpdate {
    pub plugin_id: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RecentWorkspaceFile {
    #[serde(default)]
    recent: Vec<RecentWorkspace>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self { schema_version: 1, plugins: BTreeMap::new() }
    }
}

impl WorkspaceState {
    fn new(recent_store_path: PathBuf, current: Option<WorkspaceInfo>) -> Self {
        let recent = load_recent_workspaces(&recent_store_path).unwrap_or_default();
        Self { recent_store_path, inner: RwLock::new(WorkspaceRuntimeState { current, recent }) }
    }

    fn snapshot(&self) -> Result<WorkspaceRuntimeState, String> {
        let guard =
            self.inner.read().map_err(|_| "failed to lock workspace state for read".to_string())?;
        Ok(WorkspaceRuntimeState { current: guard.current.clone(), recent: guard.recent.clone() })
    }

    fn set_current(&self, workspace: Option<WorkspaceInfo>) -> Result<(), String> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| "failed to lock workspace state for write".to_string())?;

        if let Some(workspace) = &workspace {
            upsert_recent_workspace(&mut guard.recent, workspace);
            save_recent_workspaces(&self.recent_store_path, &guard.recent)?;
        }

        guard.current = workspace;
        Ok(())
    }
}

pub fn init<R: Runtime>(app: &mut tauri::App<R>) -> Result<WorkspaceBootstrap, String> {
    let recent_store_path = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("failed to resolve app config directory: {error}"))?
        .join("workspaces.json");
    let current = startup_workspace_root().map(prepare_workspace).transpose()?;
    let sidecar_config = current.as_ref().map(sidecar_config_for_workspace).unwrap_or_default();

    app.manage(WorkspaceState::new(recent_store_path, current));

    Ok(WorkspaceBootstrap { sidecar_config })
}

#[tauri::command]
pub fn workspace_state(state: State<'_, WorkspaceState>) -> Result<WorkspaceStateResponse, String> {
    state_response(&state)
}

#[tauri::command]
pub fn workspace_open<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, WorkspaceState>,
    root_path: String,
) -> Result<WorkspaceStateResponse, String> {
    let workspace = prepare_workspace(PathBuf::from(root_path))?;
    let sidecar_config = sidecar_config_for_workspace(&workspace);
    restart_server_sidecar(&app_handle, sidecar_config)?;
    state.set_current(Some(workspace))?;
    state_response(&state)
}

#[tauri::command]
pub fn workspace_close<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, WorkspaceState>,
) -> Result<WorkspaceStateResponse, String> {
    restart_server_sidecar(&app_handle, ServerSidecarConfig::default())?;
    state.set_current(None)?;
    state_response(&state)
}

#[tauri::command]
pub fn workspace_read_directory(
    state: State<'_, WorkspaceState>,
    relative_path: Option<String>,
) -> Result<WorkspaceDirectoryResponse, String> {
    let workspace = active_workspace(&state)?;
    let relative_path = normalize_relative_path(relative_path.as_deref().unwrap_or(""))?;
    let root = PathBuf::from(&workspace.root_path);
    let directory = resolve_workspace_path(&root, &relative_path)?;
    if !directory.is_dir() {
        return Err(format!("workspace path `{relative_path}` is not a directory"));
    }

    let mut entries = Vec::new();
    let mut truncated = false;
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("failed to read directory {}: {error}", directory.display()))?
    {
        if entries.len() >= MAX_DIRECTORY_ENTRIES {
            truncated = true;
            break;
        }

        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let file_type =
            entry.file_type().map_err(|error| format!("failed to read file type: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if should_hide_entry(&name, file_type.is_dir()) {
            continue;
        }

        let entry_relative_path = join_relative_path(&relative_path, &name);
        entries.push(WorkspaceFileEntry {
            id: if entry_relative_path.is_empty() {
                name.clone()
            } else {
                entry_relative_path.clone()
            },
            name,
            relative_path: entry_relative_path,
            kind: if file_type.is_dir() {
                WorkspaceFileKind::Directory
            } else {
                WorkspaceFileKind::File
            },
            has_children: file_type.is_dir(),
        });
    }

    entries.sort_by(|left, right| match (&left.kind, &right.kind) {
        (WorkspaceFileKind::Directory, WorkspaceFileKind::File) => std::cmp::Ordering::Less,
        (WorkspaceFileKind::File, WorkspaceFileKind::Directory) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(WorkspaceDirectoryResponse { relative_path, entries, truncated })
}

#[tauri::command]
pub fn workspace_read_file(
    state: State<'_, WorkspaceState>,
    relative_path: String,
) -> Result<WorkspaceFileContent, String> {
    let workspace = active_workspace(&state)?;
    let relative_path = normalize_relative_path(&relative_path)?;
    let root = PathBuf::from(&workspace.root_path);
    let path = resolve_workspace_path(&root, &relative_path)?;
    if !path.is_file() {
        return Err(format!("workspace path `{relative_path}` is not a file"));
    }

    let metadata = fs::metadata(&path)
        .map_err(|error| format!("failed to read file metadata {}: {error}", path.display()))?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "file is too large to preview ({} bytes, limit {} bytes)",
            metadata.len(),
            MAX_FILE_BYTES
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&path)
        .map_err(|error| format!("failed to open file {}: {error}", path.display()))?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read file {}: {error}", path.display()))?;
    if bytes.contains(&0) {
        return Err("binary files cannot be previewed".to_string());
    }
    let content = String::from_utf8(bytes).map_err(|_| "file is not valid UTF-8".to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(relative_path.as_str())
        .to_owned();

    let content_hash = content_hash(content.as_bytes());

    Ok(WorkspaceFileContent {
        relative_path,
        name,
        size_bytes: metadata.len(),
        content,
        content_hash,
    })
}

#[tauri::command]
pub fn workspace_write_file(
    state: State<'_, WorkspaceState>,
    command: WorkspaceWriteFileCommand,
) -> Result<WorkspaceWriteFileView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::write_file(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_git_status(
    state: State<'_, WorkspaceState>,
) -> Result<WorkspaceGitStatusView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_status(PathBuf::from(workspace.root_path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_git_stage(
    state: State<'_, WorkspaceState>,
    command: WorkspaceGitPathCommand,
) -> Result<WorkspaceGitOperationView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_stage(PathBuf::from(workspace.root_path), &command.path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_git_unstage(
    state: State<'_, WorkspaceState>,
    command: WorkspaceGitPathCommand,
) -> Result<WorkspaceGitOperationView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_unstage(PathBuf::from(workspace.root_path), &command.path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_git_discard(
    state: State<'_, WorkspaceState>,
    command: WorkspaceGitPathCommand,
) -> Result<WorkspaceGitOperationView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_discard(PathBuf::from(workspace.root_path), &command.path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_git_commit(
    state: State<'_, WorkspaceState>,
    command: WorkspaceGitCommitCommand,
) -> Result<WorkspaceGitOperationView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_commit(PathBuf::from(workspace.root_path), &command.message)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn workspace_console_run(
    state: State<'_, WorkspaceState>,
    command: String,
) -> Result<WorkspaceConsoleOutput, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::run_console_command(PathBuf::from(workspace.root_path), &command)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_update_plugin_preference(
    state: State<'_, WorkspaceState>,
    update: WorkspacePluginPreferenceUpdate,
) -> Result<WorkspaceStateResponse, String> {
    validate_plugin_id(&update.plugin_id)?;
    let workspace = active_workspace(&state)?;
    let config_path = PathBuf::from(&workspace.workspace_config_path);
    let mut config = load_workspace_config(&config_path)?;
    if update.enabled == Some(false) {
        config.plugins.insert(update.plugin_id, WorkspacePluginConfig { enabled: Some(false) });
    } else {
        config.plugins.remove(&update.plugin_id);
    }
    write_workspace_config(&config_path, &config)?;
    state_response(&state)
}

fn state_response(state: &WorkspaceState) -> Result<WorkspaceStateResponse, String> {
    let snapshot = state.snapshot()?;
    let config = snapshot
        .current
        .as_ref()
        .map(|workspace| load_workspace_config(Path::new(&workspace.workspace_config_path)))
        .transpose()?;
    Ok(WorkspaceStateResponse { current: snapshot.current, recent: snapshot.recent, config })
}

fn active_workspace(state: &WorkspaceState) -> Result<WorkspaceInfo, String> {
    state.snapshot()?.current.ok_or_else(|| "no workspace is currently open".to_string())
}

fn prepare_workspace(root_path: PathBuf) -> Result<WorkspaceInfo, String> {
    let root = root_path.canonicalize().map_err(|error| {
        format!("failed to resolve workspace path {}: {error}", root_path.display())
    })?;
    if !root.is_dir() {
        return Err(format!("workspace path {} is not a directory", root.display()));
    }

    let slab_dir = root.join(SLAB_DIR_NAME);
    let model_config_dir = slab_dir.join("models");
    let session_state_dir = slab_dir.join("sessions");
    fs::create_dir_all(&model_config_dir).map_err(|error| {
        format!(
            "failed to create workspace models directory {}: {error}",
            model_config_dir.display()
        )
    })?;
    fs::create_dir_all(&session_state_dir).map_err(|error| {
        format!(
            "failed to create workspace sessions directory {}: {error}",
            session_state_dir.display()
        )
    })?;

    let settings_path = slab_dir.join(SETTINGS_FILE);
    let global_plugins_dir = global_plugins_root().to_string_lossy().into_owned();
    let mut settings = if settings_path.exists() {
        let raw = fs::read_to_string(&settings_path).map_err(|error| {
            format!("failed to read workspace settings {}: {error}", settings_path.display())
        })?;
        serde_json::from_str::<SettingsDocument>(&raw).map_err(|error| {
            format!("failed to parse workspace settings {}: {error}", settings_path.display())
        })?
    } else {
        SettingsDocument::default()
    };
    if settings.plugin.install_dir.as_deref() != Some(global_plugins_dir.as_str()) {
        settings.plugin.install_dir = Some(global_plugins_dir);
        write_json_file(&settings_path, &settings)?;
    }

    let workspace_config_path = slab_dir.join(WORKSPACE_CONFIG_FILE);
    if !workspace_config_path.exists() {
        write_workspace_config(&workspace_config_path, &WorkspaceConfig::default())?;
    }

    let name = root.file_name().and_then(|name| name.to_str()).unwrap_or("Workspace").to_owned();

    Ok(WorkspaceInfo {
        root_path: root.to_string_lossy().into_owned(),
        name,
        slab_dir: slab_dir.to_string_lossy().into_owned(),
        settings_path: settings_path.to_string_lossy().into_owned(),
        workspace_config_path: workspace_config_path.to_string_lossy().into_owned(),
        database_path: slab_dir.join(DATABASE_FILE).to_string_lossy().into_owned(),
        model_config_dir: model_config_dir.to_string_lossy().into_owned(),
        session_state_dir: session_state_dir.to_string_lossy().into_owned(),
    })
}

fn sidecar_config_for_workspace(workspace: &WorkspaceInfo) -> ServerSidecarConfig {
    ServerSidecarConfig {
        database_url: Some(sqlite_url_for_path(Path::new(&workspace.database_path))),
        settings_path: Some(PathBuf::from(&workspace.settings_path)),
        model_config_dir: Some(PathBuf::from(&workspace.model_config_dir)),
        session_state_dir: Some(PathBuf::from(&workspace.session_state_dir)),
        plugins_dir: Some(global_plugins_root()),
    }
}

fn startup_workspace_root() -> Option<PathBuf> {
    std::env::args_os().skip(1).find_map(folder_arg)
}

fn folder_arg(value: OsString) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    path.is_dir().then_some(path)
}

fn default_settings_path() -> PathBuf {
    std::env::var("SLAB_SETTINGS_PATH").ok().map(PathBuf::from).unwrap_or_else(|| {
        config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Slab").join("settings.json")
    })
}

fn global_plugins_root() -> PathBuf {
    plugins::resolve_plugins_root_for_settings_path(&default_settings_path())
}

fn load_recent_workspaces(path: &Path) -> Result<Vec<RecentWorkspace>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read recent workspaces {}: {error}", path.display()))?;
    let file: RecentWorkspaceFile = serde_json::from_str(&raw).map_err(|error| {
        format!("failed to parse recent workspaces {}: {error}", path.display())
    })?;
    Ok(file.recent)
}

fn save_recent_workspaces(path: &Path, recent: &[RecentWorkspace]) -> Result<(), String> {
    write_json_file(path, &RecentWorkspaceFile { recent: recent.to_vec() })
}

fn upsert_recent_workspace(recent: &mut Vec<RecentWorkspace>, workspace: &WorkspaceInfo) {
    recent.retain(|item| item.root_path != workspace.root_path);
    recent.insert(
        0,
        RecentWorkspace {
            root_path: workspace.root_path.clone(),
            name: workspace.name.clone(),
            last_opened_at: now_millis(),
        },
    );
    recent.truncate(MAX_RECENT_WORKSPACES);
}

fn load_workspace_config(path: &Path) -> Result<WorkspaceConfig, String> {
    if !path.exists() {
        return Ok(WorkspaceConfig::default());
    }
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read workspace config {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("failed to parse workspace config {}: {error}", path.display()))
}

fn write_workspace_config(path: &Path, config: &WorkspaceConfig) -> Result<(), String> {
    write_json_file(path, config)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create directory {}: {error}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize JSON for {}: {error}", path.display()))?;
    fs::write(path, format!("{content}\n"))
        .map_err(|error| format!("failed to write JSON file {}: {error}", path.display()))
}

fn normalize_relative_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let mut parts = Vec::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(segment) => {
                let segment = segment.to_string_lossy();
                if segment == SLAB_DIR_NAME {
                    return Err(
                        "workspace internals cannot be opened from the file tree".to_string()
                    );
                }
                parts.push(segment.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("workspace path `{raw}` is invalid"));
            }
        }
    }

    Ok(parts.join("/"))
}

fn resolve_workspace_path(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let candidate =
        if relative_path.is_empty() { root.to_path_buf() } else { root.join(relative_path) };
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("failed to resolve workspace root {}: {error}", root.display()))?;
    let canonical_candidate = candidate.canonicalize().map_err(|error| {
        format!("failed to resolve workspace path {}: {error}", candidate.display())
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(format!("workspace path `{relative_path}` escapes the workspace root"));
    }
    Ok(canonical_candidate)
}

fn join_relative_path(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_owned() } else { format!("{parent}/{name}") }
}

fn should_hide_entry(name: &str, is_directory: bool) -> bool {
    is_directory && IGNORED_DIR_NAMES.iter().any(|ignored| ignored.eq_ignore_ascii_case(name))
}

fn sqlite_url_for_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
    format!("{prefix}{normalized}?mode=rwc")
}

fn validate_plugin_id(plugin_id: &str) -> Result<(), String> {
    let valid = (2..=64).contains(&plugin_id.len())
        && plugin_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        });
    valid.then_some(()).ok_or_else(|| format!("invalid plugin id `{plugin_id}`"))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_relative_path, sqlite_url_for_path, validate_plugin_id};
    use std::path::Path;

    #[test]
    fn normalize_relative_path_rejects_workspace_internals() {
        assert!(normalize_relative_path(".slab/settings.json").is_err());
    }

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../secret.txt").is_err());
    }

    #[test]
    fn validate_plugin_id_accepts_manifest_style_ids() {
        assert!(validate_plugin_id("video-subtitle_translator").is_ok());
        assert!(validate_plugin_id("Plugin").is_err());
    }

    #[test]
    fn sqlite_url_for_path_uses_file_url_shape() {
        assert!(
            sqlite_url_for_path(Path::new("C:/Project/.slab/slab.db")).starts_with("sqlite:///")
        );
    }
}
