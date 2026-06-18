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
    create_file_in_active_workspace(&state, command)
}

#[tauri::command]
pub fn workspace_create_directory(
    state: State<'_, WorkspaceState>,
    command: WorkspaceCreateDirectoryCommand,
) -> Result<WorkspacePathView, String> {
    create_directory_in_active_workspace(&state, command)
}

#[tauri::command]
pub fn workspace_rename_path(
    state: State<'_, WorkspaceState>,
    command: WorkspaceRenamePathCommand,
) -> Result<WorkspacePathView, String> {
    rename_path_in_active_workspace(&state, command)
}

#[tauri::command]
pub fn workspace_delete_path(
    state: State<'_, WorkspaceState>,
    command: WorkspaceDeletePathCommand,
) -> Result<WorkspacePathView, String> {
    delete_path_in_active_workspace(&state, command)
}

fn create_file_in_active_workspace(
    state: &WorkspaceState,
    command: WorkspaceCreateFileCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(state)?;
    WorkspaceService::create_file(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

fn create_directory_in_active_workspace(
    state: &WorkspaceState,
    command: WorkspaceCreateDirectoryCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(state)?;
    WorkspaceService::create_directory(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

fn rename_path_in_active_workspace(
    state: &WorkspaceState,
    command: WorkspaceRenamePathCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(state)?;
    WorkspaceService::rename_path(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

fn delete_path_in_active_workspace(
    state: &WorkspaceState,
    command: WorkspaceDeletePathCommand,
) -> Result<WorkspacePathView, String> {
    let workspace = active_workspace(state)?;
    WorkspaceService::delete_path(PathBuf::from(workspace.root_path), command)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use slab_app_core::domain::models::{
        WorkspaceCreateDirectoryCommand, WorkspaceCreateFileCommand, WorkspaceDeletePathCommand,
        WorkspaceRenamePathCommand,
    };

    use super::{
        create_directory_in_active_workspace, create_file_in_active_workspace,
        delete_path_in_active_workspace, rename_path_in_active_workspace,
    };
    use crate::workspace::{workspace_info_for_test, workspace_state_for_test};

    #[test]
    fn file_ops_require_active_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = workspace_state_for_test(temp.path().join("recent.json"), None);

        let error = create_file_in_active_workspace(
            &state,
            WorkspaceCreateFileCommand { relative_path: "src/main.rs".to_owned() },
        )
        .expect_err("workspace should be required");

        assert_eq!(error, "no workspace is currently open");
    }

    #[test]
    fn create_file_uses_active_workspace_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = workspace_state_for_test(
            temp.path().join("recent.json"),
            Some(workspace_info_for_test(temp.path())),
        );

        let created = create_file_in_active_workspace(
            &state,
            WorkspaceCreateFileCommand { relative_path: "src/main.rs".to_owned() },
        )
        .expect("file created");

        assert_eq!(created.relative_path, "src/main.rs");
        assert!(temp.path().join("src/main.rs").is_file());
    }

    #[test]
    fn file_ops_reject_paths_outside_active_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = workspace_state_for_test(
            temp.path().join("recent.json"),
            Some(workspace_info_for_test(temp.path())),
        );

        let create_error = create_directory_in_active_workspace(
            &state,
            WorkspaceCreateDirectoryCommand { relative_path: "../outside".to_owned() },
        )
        .expect_err("directory escape should be rejected");
        assert!(create_error.contains("workspace path"));

        fs::write(temp.path().join("inside.txt"), "content").expect("seed file");
        let rename_error = rename_path_in_active_workspace(
            &state,
            WorkspaceRenamePathCommand {
                from_relative_path: "inside.txt".to_owned(),
                to_relative_path: "../outside.txt".to_owned(),
            },
        )
        .expect_err("rename escape should be rejected");
        assert!(rename_error.contains("workspace path"));

        let delete_error = delete_path_in_active_workspace(
            &state,
            WorkspaceDeletePathCommand {
                relative_path: "../outside.txt".to_owned(),
                recursive: false,
            },
        )
        .expect_err("delete escape should be rejected");
        assert!(delete_error.contains("workspace path"));
    }
}
