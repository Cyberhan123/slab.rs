use std::fs;
use std::path::PathBuf;

fn main() {
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../manifests/models/slab-manifest.schema.json");
    fs::write(&output_path, slab_model_pack::render_manifest_schema())
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", output_path.display()));
}