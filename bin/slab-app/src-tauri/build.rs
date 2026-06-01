fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "plugin_list",
            "plugin_mount_view",
            "plugin_update_view_bounds",
            "plugin_unmount_view",
            "plugin_call",
            "plugin_api_request",
            "plugin_pick_file",
            "plugin_set_theme_snapshot",
            "plugin_theme_snapshot",
            "workspace_state",
            "workspace_open",
            "workspace_close",
            "workspace_read_directory",
            "workspace_read_file",
            "workspace_stat_path",
            "workspace_search_files",
            "workspace_search_text",
            "workspace_create_file",
            "workspace_create_directory",
            "workspace_rename_path",
            "workspace_delete_path",
            "workspace_write_file",
            "workspace_git_status",
            "workspace_git_stage",
            "workspace_git_unstage",
            "workspace_git_discard",
            "workspace_git_commit",
            "workspace_git_diff",
            "workspace_console_run",
            "workspace_update_plugin_preference",
            "workspace_terminal_session",
        ]));

    tauri_build::try_build(attributes).expect("failed to run tauri build script");
}
