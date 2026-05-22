use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let workspace_root = slab_build_utils::workspace_root()?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let version = env::var("CARGO_PKG_VERSION")?;
    let target_os = env::var("CARGO_CFG_TARGET_OS")?;

    println!("cargo:rerun-if-changed={}", workspace_root.join("vendor").display());
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");

    let packaged_manifest = if target_os == "windows" {
        slab_utils::cab::build_runtime_payload_plan(&workspace_root, &version)?.packaged_manifest
    } else {
        println!(
            "cargo:warning=skipping embedded runtime payload manifest generation for target_os={target_os}; packaged CAB payloads are only built for Windows releases"
        );
        slab_utils::cab::PackagedPayloadManifest::empty(&version)
    };

    let manifest_bytes = serde_json::to_vec_pretty(&packaged_manifest)?;
    fs::write(out_dir.join("payload-manifest.json"), manifest_bytes)?;

    Ok(())
}
