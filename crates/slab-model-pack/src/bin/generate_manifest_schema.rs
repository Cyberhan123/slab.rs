use std::fs;
use std::path::PathBuf;

fn main() {
    let rendered = slab_model_pack::render_manifest_schema();

    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/public/manifests/v1/slab-manifest.schema.json");
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("failed to create {}: {error}", parent.display()));
    }
    fs::write(&output_path, &rendered)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", output_path.display()));

    let mirror_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../manifests/models/slab-manifest.schema.json");
    fs::write(&mirror_path, rendered)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", mirror_path.display()));
}
