fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "plugin_list",
            "plugin_mount_view",
            "plugin_update_view_bounds",
            "plugin_unmount_view",
            "plugin_call",
            "plugin_pick_file",
            "plugin_set_theme_snapshot",
            "plugin_theme_snapshot",
        ]));

    tauri_build::try_build(attributes).expect("failed to run tauri build script");
}
