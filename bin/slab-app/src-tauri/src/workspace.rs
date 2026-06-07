use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use slab_app_core::domain::models::{
    WorkspaceConsoleOutput, WorkspaceDirectoryView, WorkspaceFileContent, WorkspaceFileSearchView,
    WorkspaceGitCommitCommand, WorkspaceGitDiffCommand, WorkspaceGitDiffView,
    WorkspaceGitOperationView, WorkspaceGitPathCommand, WorkspaceGitStatusView,
    WorkspacePathMetadata, WorkspaceWriteFileCommand, WorkspaceWriteFileView,
};
use slab_app_core::domain::services::WorkspaceService;
use slab_config::SettingsDocument;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::paths::{display_path_string, settings_path_from_env_or_default};
use crate::setup::{ServerSidecarConfig, restart_server_sidecar};

const SLAB_DIR_NAME: &str = ".slab";
const SETTINGS_FILE: &str = "settings.json";
const MAX_RECENT_WORKSPACES: usize = 10;
pub(crate) const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_SEARCH_RESULTS: usize = 100;
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
        Self {
            schema_version: SettingsDocument::default().schema_version,
            plugins: BTreeMap::new(),
        }
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
    let recent_store_path = slab_utils::app_home::app_home_dir().join("workspaces.json");
    let current = startup_workspace_root().map(prepare_workspace).transpose()?;
    let sidecar_config =
        current.as_ref().map(sidecar_config_for_workspace).unwrap_or_else(global_sidecar_config);

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
    restart_server_sidecar(&app_handle, global_sidecar_config())?;
    state.set_current(None)?;
    state_response(&state)
}

#[tauri::command]
pub fn workspace_read_directory(
    state: State<'_, WorkspaceState>,
    relative_path: Option<String>,
    include_ignored: Option<bool>,
) -> Result<WorkspaceDirectoryView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::read_directory(
        PathBuf::from(workspace.root_path),
        relative_path.as_deref(),
        include_ignored.unwrap_or(false),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_stat_path(
    state: State<'_, WorkspaceState>,
    relative_path: String,
) -> Result<WorkspacePathMetadata, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::stat_path(PathBuf::from(workspace.root_path), &relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_read_file(
    state: State<'_, WorkspaceState>,
    relative_path: String,
) -> Result<WorkspaceFileContent, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::read_file(PathBuf::from(workspace.root_path), &relative_path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_search_files(
    state: State<'_, WorkspaceState>,
    query: String,
) -> Result<WorkspaceFileSearchView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::search_files(PathBuf::from(workspace.root_path), &query)
        .map_err(|error| error.to_string())
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
pub fn workspace_git_diff(
    state: State<'_, WorkspaceState>,
    command: WorkspaceGitDiffCommand,
) -> Result<WorkspaceGitDiffView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::git_diff(PathBuf::from(workspace.root_path), &command.path, command.staged)
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
    write_workspace_plugin_preference(Path::new(&workspace.settings_path), &update)?;
    state_response(&state)
}

fn state_response(state: &WorkspaceState) -> Result<WorkspaceStateResponse, String> {
    let snapshot = state.snapshot()?;
    let config = snapshot
        .current
        .as_ref()
        .map(|workspace| load_workspace_config(Path::new(&workspace.settings_path)))
        .transpose()?;
    Ok(WorkspaceStateResponse { current: snapshot.current, recent: snapshot.recent, config })
}

pub(crate) fn active_workspace(state: &WorkspaceState) -> Result<WorkspaceInfo, String> {
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
    fs::create_dir_all(&slab_dir).map_err(|error| {
        format!("failed to create workspace settings directory {}: {error}", slab_dir.display())
    })?;
    let settings_path = slab_dir.join(SETTINGS_FILE);
    migrate_legacy_workspace_config(&slab_dir.join("workspace.json"), &settings_path)?;
    ensure_workspace_settings_file(&settings_path)?;

    let name = root.file_name().and_then(|name| name.to_str()).unwrap_or("Workspace").to_owned();

    Ok(WorkspaceInfo {
        root_path: display_path_string(&root),
        name,
        slab_dir: display_path_string(&slab_dir),
        settings_path: display_path_string(&settings_path),
        database_path: display_path_string(&slab_utils::app_home::database_path()),
        model_config_dir: display_path_string(&slab_utils::app_home::models_dir()),
        session_state_dir: display_path_string(&slab_utils::app_home::sessions_dir()),
    })
}

fn sidecar_config_for_workspace(workspace: &WorkspaceInfo) -> ServerSidecarConfig {
    ServerSidecarConfig {
        settings_path: Some(settings_path_from_env_or_default()),
        settings_overlay_path: Some(PathBuf::from(&workspace.settings_path)),
        workspace_root: Some(PathBuf::from(&workspace.root_path)),
        ..ServerSidecarConfig::default()
    }
}

fn global_sidecar_config() -> ServerSidecarConfig {
    ServerSidecarConfig {
        settings_path: Some(settings_path_from_env_or_default()),
        ..ServerSidecarConfig::default()
    }
}

fn startup_workspace_root() -> Option<PathBuf> {
    std::env::args_os().skip(1).find_map(folder_arg)
}

fn folder_arg(value: OsString) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    path.is_dir().then_some(path)
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

fn ensure_workspace_settings_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        write_json_file(path, &Value::Object(Map::new()))?;
    }
    Ok(())
}

fn migrate_legacy_workspace_config(legacy_path: &Path, settings_path: &Path) -> Result<(), String> {
    if !legacy_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(legacy_path).map_err(|error| {
        format!("failed to read legacy workspace config {}: {error}", legacy_path.display())
    })?;
    let legacy: WorkspaceConfig = serde_json::from_str(&raw).map_err(|error| {
        format!("failed to parse legacy workspace config {}: {error}", legacy_path.display())
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

fn load_workspace_config(path: &Path) -> Result<WorkspaceConfig, String> {
    let settings = load_settings_overlay(path)?;
    workspace_config_from_settings_overlay(&settings)
}

fn load_settings_overlay(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = fs::read_to_string(path).map_err(|error| {
        format!("failed to read workspace settings {}: {error}", path.display())
    })?;
    let value: Value = serde_json::from_str(&raw).map_err(|error| {
        format!("failed to parse workspace settings {}: {error}", path.display())
    })?;
    if !value.is_object() {
        return Err(format!("workspace settings {} must contain a JSON object", path.display()));
    }
    Ok(value)
}

fn workspace_config_from_settings_overlay(settings: &Value) -> Result<WorkspaceConfig, String> {
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
                    format!("workspace plugin preference `{plugin_id}` has invalid shape: {error}")
                })?;
            if config.enabled.is_some() {
                plugins.insert(plugin_id.clone(), config);
            }
        }
    }

    Ok(WorkspaceConfig { schema_version, plugins })
}

fn write_workspace_plugin_preference(
    settings_path: &Path,
    update: &WorkspacePluginPreferenceUpdate,
) -> Result<(), String> {
    let mut settings = load_settings_overlay(settings_path)?;
    if update.enabled == Some(false) {
        set_workspace_plugin_enabled(&mut settings, &update.plugin_id, false);
    } else {
        remove_workspace_plugin_preference(&mut settings, &update.plugin_id);
    }
    prune_empty_objects(&mut settings);
    write_json_file(settings_path, &settings)
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

pub(crate) fn join_relative_path(parent: &str, name: &str) -> String {
    if parent.is_empty() { name.to_owned() } else { format!("{parent}/{name}") }
}

pub(crate) fn should_hide_entry(name: &str, is_directory: bool, include_ignored: bool) -> bool {
    !include_ignored
        && is_directory
        && IGNORED_DIR_NAMES.iter().any(|ignored| ignored.eq_ignore_ascii_case(name))
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

#[cfg(test)]
mod tests {
    use super::validate_plugin_id;

    #[test]
    fn validate_plugin_id_accepts_manifest_style_ids() {
        assert!(validate_plugin_id("video-subtitle_translator").is_ok());
        assert!(validate_plugin_id("Plugin").is_err());
    }
}
