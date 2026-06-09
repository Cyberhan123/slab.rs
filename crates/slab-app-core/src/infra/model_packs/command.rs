use std::path::Path;

use slab_model_pack::{
    ModelPackManifest, ModelPackRuntimeBridge, PackModelStatus, PackPricing, PackRuntimePresets,
    PackSource,
};

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelSpec, Pricing, RuntimePresets,
    UnifiedModelKind, UnifiedModelStatus,
};
use crate::error::AppCoreError;

use super::map_model_pack_error;

pub(super) fn build_model_command(
    path: &Path,
    manifest: &ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
) -> Result<CreateModelCommand, AppCoreError> {
    match manifest.sources.first().map(|candidate| &candidate.source) {
        Some(PackSource::Cloud { provider_id, remote_model_id }) => {
            build_cloud_model_command(manifest, provider_id, remote_model_id)
        }
        _ => build_local_model_command(path, manifest, resolved),
    }
}

fn default_status_for_runtime_bridge(bridge: &ModelPackRuntimeBridge) -> UnifiedModelStatus {
    match bridge.model_spec.source {
        slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
        _ => UnifiedModelStatus::Ready,
    }
}

fn build_runtime_presets(options: &slab_types::JsonOptions) -> Option<RuntimePresets> {
    let max_tokens = options.get("max_tokens").and_then(value_to_u32);
    let temperature = options.get("temperature").and_then(value_to_f32);
    let top_p = options.get("top_p").and_then(value_to_f32);
    let top_k = options.get("top_k").and_then(value_to_i32);
    let min_p = options.get("min_p").and_then(value_to_f32);
    let presence_penalty = options.get("presence_penalty").and_then(value_to_f32);
    let repetition_penalty = options.get("repetition_penalty").and_then(value_to_f32);

    (max_tokens.is_some()
        || temperature.is_some()
        || top_p.is_some()
        || top_k.is_some()
        || min_p.is_some()
        || presence_penalty.is_some()
        || repetition_penalty.is_some())
    .then_some(RuntimePresets {
        max_tokens,
        temperature,
        top_p,
        top_k,
        min_p,
        presence_penalty,
        repetition_penalty,
    })
}

fn value_to_f32(value: &serde_json::Value) -> Option<f32> {
    value.as_f64().map(|value| value as f32)
}

fn value_to_u32(value: &serde_json::Value) -> Option<u32> {
    value.as_u64().and_then(|value| u32::try_from(value).ok())
}

fn value_to_i32(value: &serde_json::Value) -> Option<i32> {
    value.as_i64().and_then(|value| i32::try_from(value).ok())
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
    let status = manifest_status(manifest.status)
        .unwrap_or_else(|| default_status_for_runtime_bridge(&bridge));
    let runtime_presets = build_runtime_presets_from_manifest(manifest.runtime_presets.as_ref())
        .or_else(|| build_runtime_presets(&bridge.inference_defaults));
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

fn build_cloud_model_command(
    manifest: &ModelPackManifest,
    provider_id: &str,
    remote_model_id: &str,
) -> Result<CreateModelCommand, AppCoreError> {
    let provider_id = normalize_required_manifest_text(provider_id, "source.provider_id")?;
    let remote_model_id =
        normalize_required_manifest_text(remote_model_id, "source.remote_model_id")?;

    Ok(CreateModelCommand {
        id: Some(manifest.id.clone()),
        display_name: manifest.label.clone(),
        kind: UnifiedModelKind::Cloud,
        backend_id: None,
        capabilities: Some(manifest.capabilities.clone()),
        status: manifest_status(manifest.status),
        spec: ModelSpec {
            provider_id: Some(provider_id),
            remote_model_id: Some(remote_model_id),
            pricing: build_pricing_from_manifest(manifest.pricing.as_ref()),
            context_window: manifest.context_window,
            ..Default::default()
        },
        runtime_presets: build_runtime_presets_from_manifest(manifest.runtime_presets.as_ref()),
    })
}

fn manifest_status(status: Option<PackModelStatus>) -> Option<UnifiedModelStatus> {
    status.map(|status| match status {
        PackModelStatus::Ready => UnifiedModelStatus::Ready,
        PackModelStatus::NotDownloaded => UnifiedModelStatus::NotDownloaded,
        PackModelStatus::Downloading => UnifiedModelStatus::Downloading,
        PackModelStatus::Error => UnifiedModelStatus::Error,
    })
}

fn build_pricing_from_manifest(pricing: Option<&PackPricing>) -> Option<Pricing> {
    pricing.map(|pricing| Pricing { input: pricing.input, output: pricing.output })
}

fn build_runtime_presets_from_manifest(
    runtime_presets: Option<&PackRuntimePresets>,
) -> Option<RuntimePresets> {
    let runtime_presets = runtime_presets?;
    (runtime_presets.max_tokens.is_some()
        || runtime_presets.temperature.is_some()
        || runtime_presets.top_p.is_some()
        || runtime_presets.top_k.is_some()
        || runtime_presets.min_p.is_some()
        || runtime_presets.presence_penalty.is_some()
        || runtime_presets.repetition_penalty.is_some())
    .then_some(RuntimePresets {
        max_tokens: runtime_presets.max_tokens,
        temperature: runtime_presets.temperature,
        top_p: runtime_presets.top_p,
        top_k: runtime_presets.top_k,
        min_p: runtime_presets.min_p,
        presence_penalty: runtime_presets.presence_penalty,
        repetition_penalty: runtime_presets.repetition_penalty,
    })
}

fn normalize_optional_manifest_text(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn normalize_required_manifest_text(value: &str, label: &str) -> Result<String, AppCoreError> {
    normalize_optional_manifest_text(Some(value))
        .ok_or_else(|| AppCoreError::BadRequest(format!("{} must not be empty", label)))
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
