use std::path::Path;

use slab_model_pack::{ModelPackManifest, ModelPackRuntimeBridge, PackPricing, PackSource};

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelSpec, Pricing, UnifiedModelKind,
    UnifiedModelStatus,
};
use crate::error::AppCoreError;

use super::map_model_pack_error;

pub(super) fn build_model_command(
    path: &Path,
    manifest: &ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
) -> Result<CreateModelCommand, AppCoreError> {
    // Packs are local-only; cloud-vendor models are configured via `slab-cloud-provider` and
    // activated as `UnifiedModel` rows rather than imported as model packs.
    build_local_model_command(path, manifest, resolved)
}

fn default_status_for_runtime_bridge(bridge: &ModelPackRuntimeBridge) -> UnifiedModelStatus {
    match bridge.model_spec.source {
        slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
        _ => UnifiedModelStatus::Ready,
    }
}

fn build_local_model_command(
    _path: &Path,
    manifest: &ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
) -> Result<CreateModelCommand, AppCoreError> {
    let bridge = resolved.compile_default_runtime_bridge().map_err(map_model_pack_error)?;
    let backend_id = ManagedModelBackendId::try_from(bridge.backend).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "model pack backend '{}' is not supported by managed local models: {}",
            bridge.backend, error
        ))
    })?;
    let status = default_status_for_runtime_bridge(&bridge);
    let runtime_presets =
        crate::domain::models::RuntimePresets::from_json_options(&bridge.inference_defaults);
    let (repo_id, filename, local_path) = local_source_fields(resolved, &bridge);
    let allow_local_path_fallback = repo_id.is_none();

    Ok(CreateModelCommand {
        id: Some(manifest.id.clone()),
        display_name: manifest.label.clone(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(backend_id),
        capabilities: Some(manifest.capabilities.clone()),
        status: Some(status),
        spec: ModelSpec {
            pricing: build_pricing_from_manifest(manifest.pricing.as_ref()),
            repo_id,
            filename,
            local_path: local_path.or_else(|| {
                allow_local_path_fallback
                    .then(|| {
                        bridge
                            .model_spec
                            .source
                            .primary_path()
                            .map(|value| value.to_string_lossy().into_owned())
                    })
                    .flatten()
            }),
            context_window: manifest.context_window.or(bridge.load_defaults.context_length),
            ..Default::default()
        },
        runtime_presets,
    })
}

fn build_pricing_from_manifest(pricing: Option<&PackPricing>) -> Option<Pricing> {
    pricing.map(|pricing| Pricing { input: pricing.input, output: pricing.output })
}

fn local_source_fields(
    resolved: &slab_model_pack::ResolvedModelPack,
    bridge: &ModelPackRuntimeBridge,
) -> (Option<String>, Option<String>, Option<String>) {
    let source = resolved
        .default_preset()
        .and_then(|preset| {
            preset.variant.effective_sources.first().map(|candidate| &candidate.source).or_else(
                || {
                    preset
                        .variant
                        .components
                        .get("model")
                        .map(|component| &component.document.source)
                        .or_else(|| {
                            preset
                                .variant
                                .components
                                .values()
                                .next()
                                .map(|component| &component.document.source)
                        })
                },
            )
        })
        .or_else(|| resolved.manifest.sources.first().map(|candidate| &candidate.source));

    match source {
        Some(source @ (PackSource::HuggingFace { .. } | PackSource::ModelScope { .. })) => {
            let remote_source = source
                .remote_repository()
                .expect("remote source candidates expose repository info");
            let filename = remote_source
                .files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| remote_source.files.first())
                .map(|file| file.path.clone());
            (Some(remote_source.repo_id.to_owned()), filename, None)
        }
        Some(PackSource::LocalPath { path }) => (None, None, Some(path.clone())),
        Some(PackSource::LocalFiles { files }) => {
            let local_path = files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone());
            (None, None, local_path)
        }
        _ => (
            None,
            None,
            bridge
                .model_spec
                .source
                .primary_path()
                .map(|value| value.to_string_lossy().into_owned()),
        ),
    }
}
