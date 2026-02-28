use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // 2. 定位 Workspace 编译产物目录 (target/debug 或 target/release)
    let workspace_target_dir = get_workspace_target_dir(&manifest_dir, &profile);

    // 3. 执行 Sidecar 搬运任务
    // 如果你有多个 Sidecar，可以在这里重复调用此函数
    sync_sidecar("slab-server", &target, &workspace_target_dir, &manifest_dir);
    tauri_build::build()
}

fn get_workspace_target_dir(manifest_dir: &Path, profile: &str) -> PathBuf {
    manifest_dir
        .parent()
        .expect("Failed to find tauri-app dir")
        .parent()
        .expect("Failed to find workspace root")
        .join("target")
        .join(profile)
}

/// sync_sidercar takes the compiled binary from the workspace target directory and copies it to the Tauri sidecar directory with a target-specific name.
fn sync_sidecar(bin_name: &str, target: &str, src_dir: &Path, tauri_dir: &Path) {
    let extension = if target.contains("windows") { ".exe" } else { "" };
    
    
    let src_path = src_dir.join(format!("{}{}", bin_name, extension));
    
    
    let sidecar_dir = tauri_dir.join("binaries");
    let dst_path = sidecar_dir.join(format!("{}-{}{}", bin_name, target, extension));


    if !sidecar_dir.exists() {
        fs::create_dir_all(&sidecar_dir).expect("Failed to create binaries directory");
    }

    
    if src_path.exists() {
        fs::copy(&src_path, &dst_path).expect("Failed to copy sidecar binary");
        println!("cargo:rerun-if-changed={}", src_path.to_str().unwrap());
    } else {
        println!(
            "cargo:warning=Sidecar [{}] not found at {:?}. It's normal for the first build.", 
            bin_name, src_path
        );
    }
}