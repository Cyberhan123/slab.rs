mod cabinet;
mod detect;
mod fsops;
mod ggml_manifest;
mod payload;

pub use cabinet::{create_cab, expand_cab_with_progress};
pub use detect::detect_best_variant;
pub use fsops::{
    apply_payload_manifest, apply_selected_payload, bytes_to_hex, collect_files_recursive,
    ensure_parent_dir, hash_reader, normalize_relative_path, read_json, remove_dir_if_exists,
    sha256_file, validate_relative_path, write_json,
};
pub use payload::{
    CabPackage, PAYLOAD_MANIFEST_FILE_NAME, PackagedPayloadFile, PackagedPayloadManifest,
    PackagedPayloadPackage, RequestedVariant, ResolvedPayloadFile, RuntimePayloadPlan,
    RuntimeVariant, SelectedPayloadFile, SelectedPayloadManifest, StagedRuntimePackage,
    StagedRuntimePayloads, build_runtime_payload_plan, selected_packages, stage_runtime_payloads,
};
