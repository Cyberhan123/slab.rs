use std::env;
use std::path::PathBuf;

use slab_build_utils::{
    sync_tauri_bundled_plugins, sync_tauri_sidecar, sync_tauri_vendor_runtime_artifacts,
    workspace_target_dir,
};

fn main() {
    let target = env::var("TARGET").expect("missing TARGET");
    let profile = env::var("PROFILE").expect("missing PROFILE");
    let skip_vendor_runtime_sync =
        env::var("SLAB_SKIP_VENDOR_RUNTIME_SYNC").is_ok_and(|value| value == "1");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let workspace_target_dir =
        workspace_target_dir(&profile).expect("failed to resolve workspace target directory");

    println!("cargo:rerun-if-env-changed=SLAB_SKIP_VENDOR_RUNTIME_SYNC");

    sync_tauri_sidecar("slab-server", &target, &workspace_target_dir, &manifest_dir)
        .expect("failed to sync slab-server sidecar");
    sync_tauri_sidecar("slab-runtime", &target, &workspace_target_dir, &manifest_dir)
        .expect("failed to sync slab-runtime sidecar");
    sync_tauri_bundled_plugins(&manifest_dir).expect("failed to sync bundled plugins");
    if !skip_vendor_runtime_sync {
        sync_tauri_vendor_runtime_artifacts(&target, &manifest_dir)
            .expect("failed to sync vendored runtime artifacts");
    }

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
        ]));

    tauri_build::try_build(attributes).expect("failed to run tauri build script");
}
