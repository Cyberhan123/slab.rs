use std::ffi::OsString;
use std::path::PathBuf;

use slab_app_core::domain::services::WorkspaceService;
use slab_utils::path::absolute::canonicalize_existing_preserving_symlinks;
use tauri::Runtime;

use crate::paths::settings_path_from_env_or_default;
use crate::setup::ServerSidecarConfig;

#[derive(Clone, Debug)]
pub struct WorkspaceBootstrap {
    pub sidecar_config: ServerSidecarConfig,
}

#[derive(Clone, Debug)]
struct PreparedWorkspace {
    root_path: PathBuf,
    settings_path: PathBuf,
}

pub fn init<R: Runtime>(_app: &mut tauri::App<R>) -> Result<WorkspaceBootstrap, String> {
    let current = startup_workspace_root().map(prepare_workspace).transpose()?;
    let sidecar_config =
        current.as_ref().map(sidecar_config_for_workspace).unwrap_or_else(global_sidecar_config);

    Ok(WorkspaceBootstrap { sidecar_config })
}

fn prepare_workspace(root_path: PathBuf) -> Result<PreparedWorkspace, String> {
    let root = canonicalize_existing_preserving_symlinks(&root_path).map_err(|error| {
        format!("failed to resolve workspace path {}: {error}", root_path.display())
    })?;
    if !root.is_dir() {
        return Err(format!("workspace path {} is not a directory", root.display()));
    }

    let settings_path =
        WorkspaceService::ensure_workspace_settings(&root).map_err(|error| error.to_string())?;

    Ok(PreparedWorkspace { root_path: root, settings_path })
}

fn sidecar_config_for_workspace(workspace: &PreparedWorkspace) -> ServerSidecarConfig {
    ServerSidecarConfig {
        settings_path: Some(settings_path_from_env_or_default()),
        settings_overlay_path: Some(workspace.settings_path.clone()),
        workspace_root: Some(workspace.root_path.clone()),
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

#[cfg(test)]
mod tests {
    use super::{folder_arg, prepare_workspace, sidecar_config_for_workspace};

    #[test]
    fn folder_arg_accepts_existing_directory() {
        let temp = tempfile::tempdir().expect("tempdir");

        let selected = folder_arg(temp.path().as_os_str().to_os_string());

        assert_eq!(selected.as_deref(), Some(temp.path()));
    }

    #[test]
    fn folder_arg_ignores_missing_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing");

        assert!(folder_arg(missing.into_os_string()).is_none());
    }

    #[test]
    fn prepare_workspace_creates_settings_overlay_and_sidecar_config() {
        let temp = tempfile::tempdir().expect("tempdir");

        let workspace = prepare_workspace(temp.path().to_path_buf()).expect("workspace prepared");
        let config = sidecar_config_for_workspace(&workspace);

        assert!(workspace.settings_path.is_file());
        assert_eq!(workspace.settings_path, temp.path().join(".slab").join("settings.json"));
        assert_eq!(
            config.settings_overlay_path.as_deref(),
            Some(workspace.settings_path.as_path())
        );
        assert_eq!(config.workspace_root.as_deref(), Some(workspace.root_path.as_path()));
    }
}
