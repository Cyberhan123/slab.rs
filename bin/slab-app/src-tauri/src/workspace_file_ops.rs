use std::path::PathBuf;

use slab_app_core::domain::models::{
    WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand, WorkspaceDeletePathCommand,
    WorkspacePathView, WorkspaceRenamePathCommand,
};
use slab_app_core::domain::services::WorkspaceService;
use tauri::State;

use crate::workspace::{WorkspaceState, active_workspace};

#[tauri::command]
pub fn workspace_create_file(
    state: State<'_, WorkspaceState>,
    command: WorkspaceCreateFileCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::create_file(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_create_directory(
    state: State<'_, WorkspaceState>,
    command: WorkspaceCreateDirectoryCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::create_directory(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_rename_path(
    state: State<'_, WorkspaceState>,
    command: WorkspaceRenamePathCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::rename_path(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn workspace_delete_path(
    state: State<'_, WorkspaceState>,
    command: WorkspaceDeletePathCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(&state)?;
    WorkspaceService::delete_path(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}
