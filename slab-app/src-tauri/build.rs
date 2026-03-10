use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").expect("missing TARGET");
    let profile = env::var("PROFILE").expect("missing PROFILE");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let workspace_target_dir = get_workspace_target_dir(&manifest_dir, &profile);

    sync_sidecar("slab-server", &target, &workspace_target_dir, &manifest_dir);
    sync_sidecar(
        "slab-runtime",
        &target,
        &workspace_target_dir,
        &manifest_dir,
    );

    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
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
        ]),
    );

    tauri_build::try_build(attributes).expect("failed to run tauri build script");
}

fn get_workspace_target_dir(manifest_dir: &Path, profile: &str) -> PathBuf {
    manifest_dir
        .parent()
        .expect("failed to find slab-app dir")
        .parent()
        .expect("failed to find workspace root")
        .join("target")
        .join(profile)
}

fn sync_sidecar(bin_name: &str, target: &str, src_dir: &Path, tauri_dir: &Path) {
    let extension = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let src_path = src_dir.join(format!("{bin_name}{extension}"));

    let sidecar_dir = tauri_dir.join("binaries");
    let dst_path = sidecar_dir.join(format!("{bin_name}-{target}{extension}"));

    if !sidecar_dir.exists() {
        fs::create_dir_all(&sidecar_dir).expect("failed to create binaries directory");
    }

    if src_path.exists() {
        fs::copy(&src_path, &dst_path).expect("failed to copy sidecar binary");
        println!("cargo:rerun-if-changed={}", src_path.to_string_lossy());
    } else {
        println!(
            "cargo:warning=Sidecar [{}] not found at {}. Build it before packaging.",
            bin_name,
            src_path.to_string_lossy()
        );
    }
}
