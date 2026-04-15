use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let workspace_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or("failed to resolve workspace root from CARGO_MANIFEST_DIR")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let version = env::var("CARGO_PKG_VERSION")?;

    println!("cargo:rerun-if-changed={}", workspace_root.join("vendor").display());

    let payload_plan = slab_utils::cab::build_runtime_payload_plan(workspace_root, &version)?;
    let manifest_bytes = serde_json::to_vec_pretty(&payload_plan.packaged_manifest)?;
    fs::write(out_dir.join("payload-manifest.json"), manifest_bytes)?;

    Ok(())
}
