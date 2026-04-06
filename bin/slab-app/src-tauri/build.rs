use std::env;
use std::path::PathBuf;

use slab_build_utils::{
    sync_tauri_sidecar, sync_tauri_vendor_runtime_artifacts, workspace_target_dir,
};

fn main() {
    let target = env::var("TARGET").expect("missing TARGET");
    let profile = env::var("PROFILE").expect("missing PROFILE");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let workspace_target_dir =
        workspace_target_dir(&profile).expect("failed to resolve workspace target directory");

    sync_tauri_sidecar("slab-server", &target, &workspace_target_dir, &manifest_dir)
        .expect("failed to sync slab-server sidecar");
    sync_tauri_sidecar("slab-runtime", &target, &workspace_target_dir, &manifest_dir)
        .expect("failed to sync slab-runtime sidecar");
    sync_tauri_vendor_runtime_artifacts(&target, &manifest_dir)
        .expect("failed to sync vendored runtime artifacts");

    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "greet",
            "get_api_url",
            "check_backend_status",
            "get_system_info",
            "plugin_list",
            "plugin_mount_view",
            "plugin_update_view_bounds",
            "plugin_unmount_view",
            "plugin_call",
            "plugin_api_request",
            "health",
            "transcribe",
            "list_chat_models",
            "chat_completions",
            "chat_completions_stream",
            "completions",
            "list_models",
            "create_model",
            "import_model_pack",
            "get_model",
            "update_model",
            "delete_model",
            "load_model",
            "unload_model",
            "switch_model",
            "download_model",
            "list_available_models",
            "list_sessions",
            "create_session",
            "delete_session",
            "list_session_messages",
            "list_tasks",
            "get_task",
            "get_task_result",
            "cancel_task",
            "setup_status",
            "download_ffmpeg",
            "complete_setup",
            "backend_status",
            "list_backends",
            "download_backend_lib",
            "generate_images",
            "reload_backend_lib",
            "gpu_status",
            "list_settings",
            "get_setting",
            "update_setting",
        ]));

    tauri_build::try_build(attributes).expect("failed to run tauri build script");
}
