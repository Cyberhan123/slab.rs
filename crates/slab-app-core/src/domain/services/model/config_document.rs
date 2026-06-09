use std::path::PathBuf;

use serde_json::{Map, Value};
use slab_types::ModelSource;

use crate::domain::models::{
    ModelConfigFieldScope, ModelConfigFieldView, ModelConfigOrigin, ModelConfigPresetOption,
    ModelConfigSelectionView, ModelConfigSourceArtifact, ModelConfigSourceSummary,
    ModelConfigValueType, ModelConfigVariantOption, ModelPackSelection, UnifiedModel,
};
use crate::error::AppCoreError;

use super::pack;

pub(super) fn build_model_config_summary_fields(
    model: &UnifiedModel,
    display_name: &str,
    backend_label: String,
    backend_description: &str,
) -> Result<Vec<ModelConfigFieldView>, AppCoreError> {
    Ok(vec![
        build_model_config_field(
            "model.id",
            ModelConfigFieldScope::Summary,
            "Model ID",
            Some("Catalog identifier projected from the pack manifest.".into()),
            ModelConfigValueType::String,
            Value::String(model.id.clone()),
            ModelConfigOrigin::PackManifest,
        ),
        build_model_config_field(
            "model.display_name",
            ModelConfigFieldScope::Summary,
            "Display Name",
            Some("Read-only label from the pack manifest.".into()),
            ModelConfigValueType::String,
            Value::String(display_name.to_owned()),
            ModelConfigOrigin::PackManifest,
        ),
        build_model_config_field(
            "model.backend",
            ModelConfigFieldScope::Summary,
            "Backend",
            Some(backend_description.to_owned()),
            ModelConfigValueType::String,
            Value::String(backend_label),
            ModelConfigOrigin::Derived,
        ),
        build_model_config_field(
            "model.status",
            ModelConfigFieldScope::Summary,
            "Catalog Status",
            Some("Current projected status in the models table.".into()),
            ModelConfigValueType::String,
            Value::String(model.status.as_str().to_owned()),
            ModelConfigOrigin::Derived,
        ),
        build_model_config_field(
            "model.capabilities",
            ModelConfigFieldScope::Summary,
            "Capabilities",
            Some("Capabilities declared by the pack and projected into the catalog.".into()),
            ModelConfigValueType::Json,
            serde_json::to_value(&model.capabilities).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to serialize model capabilities for config document: {error}"
                ))
            })?,
            ModelConfigOrigin::PackManifest,
        ),
    ])
}

pub(super) fn build_model_config_source_fields(
    source_summary: &ModelConfigSourceSummary,
    source_origin: ModelConfigOrigin,
) -> Vec<ModelConfigFieldView> {
    let mut fields = vec![build_model_config_field(
        "source.kind",
        ModelConfigFieldScope::Source,
        "Source Kind",
        Some("Where the selected preset resolves its artifacts from.".into()),
        ModelConfigValueType::String,
        Value::String(source_summary.source_kind.clone()),
        source_origin,
    )];
    if let Some(repo_id) = source_summary.repo_id.as_ref() {
        fields.push(build_model_config_field(
            "source.repo_id",
            ModelConfigFieldScope::Source,
            "Repo ID",
            Some("Resolved Hugging Face repository for the selected model source.".into()),
            ModelConfigValueType::String,
            Value::String(repo_id.clone()),
            source_origin,
        ));
    }
    if let Some(filename) = source_summary.filename.as_ref() {
        fields.push(build_model_config_field(
            "source.filename",
            ModelConfigFieldScope::Source,
            "Primary Artifact",
            Some("Primary artifact path selected for this preset/variant.".into()),
            ModelConfigValueType::Path,
            Value::String(filename.clone()),
            source_origin,
        ));
    }
    if let Some(local_path) = source_summary.local_path.as_ref() {
        fields.push(build_model_config_field(
            "source.local_path",
            ModelConfigFieldScope::Source,
            "Local Path",
            Some("Projected local path currently associated with the selected source.".into()),
            ModelConfigValueType::Path,
            Value::String(local_path.clone()),
            source_origin,
        ));
    }
    for artifact in &source_summary.artifacts {
        fields.push(build_model_config_field(
            format!("source.artifacts.{}", artifact.id),
            ModelConfigFieldScope::Source,
            artifact.label.clone(),
            Some("Resolved artifact path for the selected source.".into()),
            ModelConfigValueType::Path,
            Value::String(artifact.value.clone()),
            source_origin,
        ));
    }
    fields
}

pub(super) fn build_model_config_inference_fields(
    resolved: &slab_model_pack::ResolvedModelPack,
    resolved_inference_spec: &Value,
) -> Vec<ModelConfigFieldView> {
    let runtime_presets = resolved.manifest.runtime_presets.as_ref();
    let mut fields = Vec::new();
    for (path, label, description, from_manifest) in [
        (
            "temperature",
            "Temperature",
            "Resolved sampling temperature exposed by the pack.",
            runtime_presets.and_then(|value| value.temperature).is_some(),
        ),
        (
            "top_p",
            "Top P",
            "Resolved nucleus sampling value exposed by the pack.",
            runtime_presets.and_then(|value| value.top_p).is_some(),
        ),
    ] {
        if !from_manifest && !value_is_present(resolved_inference_spec, path) {
            continue;
        }
        fields.push(build_model_config_field(
            format!("inference.{path}"),
            ModelConfigFieldScope::Inference,
            label,
            Some(description.into()),
            ModelConfigValueType::Number,
            json_property_or_null(resolved_inference_spec, path),
            if from_manifest {
                ModelConfigOrigin::PackManifest
            } else {
                ModelConfigOrigin::SelectedBackendConfig
            },
        ));
    }
    fields
}

pub(super) fn build_model_config_selection_view(
    resolved: &slab_model_pack::ResolvedModelPack,
    explicit_selection: &ModelPackSelection,
    effective_selection: &ModelPackSelection,
) -> ModelConfigSelectionView {
    let default_selection = pack::default_model_pack_selection(resolved);
    let presets = resolved
        .presets
        .values()
        .map(|preset| ModelConfigPresetOption {
            id: preset.document.id.clone(),
            label: preset.document.label.clone(),
            description: preset.document.description.clone(),
            variant_id: preset
                .document
                .variant_id
                .clone()
                .or_else(|| pack::non_empty_variant_id(&preset.variant.document.id)),
            is_default: resolved.default_preset_id.as_deref() == Some(preset.document.id.as_str()),
        })
        .collect();
    let variants = resolved
        .variants
        .values()
        .map(|variant| {
            let source_preview =
                pack::source_preview_from_pack_source(variant.effective_sources.first());
            ModelConfigVariantOption {
                id: variant.document.id.clone(),
                label: variant.document.label.clone(),
                description: variant.document.description.clone(),
                repo_id: source_preview.repo_id,
                filename: source_preview.filename,
                local_path: source_preview.local_path,
                is_default: default_selection.variant_id.as_deref()
                    == Some(variant.document.id.as_str()),
            }
        })
        .collect();

    ModelConfigSelectionView {
        default_preset_id: default_selection.preset_id.clone(),
        default_variant_id: default_selection.variant_id.clone(),
        selected_preset_id: explicit_selection.preset_id.clone(),
        selected_variant_id: explicit_selection.variant_id.clone(),
        effective_preset_id: effective_selection.preset_id.clone(),
        effective_variant_id: effective_selection.variant_id.clone(),
        presets,
        variants,
    }
}

pub(super) fn build_model_config_source_summary(source: &ModelSource) -> ModelConfigSourceSummary {
    match source {
        ModelSource::HuggingFace { repo_id, files, .. } => ModelConfigSourceSummary {
            source_kind: "hugging_face".into(),
            repo_id: Some(repo_id.clone()),
            filename: files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            local_path: None,
            artifacts: source
                .files()
                .into_iter()
                .map(|(id, path)| ModelConfigSourceArtifact {
                    label: humanize_artifact_label(&id),
                    id,
                    value: path.to_string_lossy().into_owned(),
                })
                .collect(),
        },
        ModelSource::LocalPath { path } => ModelConfigSourceSummary {
            source_kind: "local_path".into(),
            repo_id: None,
            filename: None,
            local_path: Some(path.to_string_lossy().into_owned()),
            artifacts: vec![ModelConfigSourceArtifact {
                id: "model".into(),
                label: "Model".into(),
                value: path.to_string_lossy().into_owned(),
            }],
        },
        ModelSource::LocalArtifacts { .. } => ModelConfigSourceSummary {
            source_kind: "local_artifacts".into(),
            repo_id: None,
            filename: None,
            local_path: source.primary_path().map(|path| path.to_string_lossy().into_owned()),
            artifacts: source
                .files()
                .into_iter()
                .map(|(id, path)| ModelConfigSourceArtifact {
                    label: humanize_artifact_label(&id),
                    id,
                    value: path.to_string_lossy().into_owned(),
                })
                .collect(),
        },
        _ => ModelConfigSourceSummary {
            source_kind: "unknown".into(),
            repo_id: None,
            filename: None,
            local_path: None,
            artifacts: Vec::new(),
        },
    }
}

pub(super) fn build_model_config_field(
    path: impl Into<String>,
    scope: ModelConfigFieldScope,
    label: impl Into<String>,
    description_md: Option<String>,
    value_type: ModelConfigValueType,
    effective_value: Value,
    origin: ModelConfigOrigin,
) -> ModelConfigFieldView {
    ModelConfigFieldView {
        path: path.into(),
        scope,
        label: label.into(),
        description_md,
        value_type,
        effective_value,
        origin,
        editable: false,
        locked: true,
        json_schema: None,
    }
}

pub(super) fn model_source_origin(
    selected_preset: &slab_model_pack::ResolvedPreset,
) -> ModelConfigOrigin {
    if !selected_preset.variant.document.sources.is_empty()
        || !selected_preset.variant.components.is_empty()
    {
        ModelConfigOrigin::SelectedVariant
    } else {
        ModelConfigOrigin::PackManifest
    }
}

pub(super) fn diffusion_load_origin(
    bridge: &slab_model_pack::ModelPackRuntimeBridge,
    field: &str,
) -> ModelConfigOrigin {
    let Some(diffusion) = bridge.load_defaults.diffusion.as_ref() else {
        return ModelConfigOrigin::PmidFallback;
    };

    let from_pack = match field {
        "diffusion_model_path" => diffusion.diffusion_model_path.is_some(),
        "vae_path" => diffusion.vae_path.is_some(),
        "taesd_path" => diffusion.taesd_path.is_some(),
        "clip_l_path" => diffusion.clip_l_path.is_some(),
        "clip_g_path" => diffusion.clip_g_path.is_some(),
        "t5xxl_path" => diffusion.t5xxl_path.is_some(),
        "flash_attn" => diffusion.flash_attn,
        "vae_device" => !diffusion.vae_device.is_empty(),
        "clip_device" => !diffusion.clip_device.is_empty(),
        "offload_params_to_cpu" => diffusion.offload_params_to_cpu,
        _ => false,
    };

    if from_pack {
        ModelConfigOrigin::SelectedBackendConfig
    } else {
        ModelConfigOrigin::PmidFallback
    }
}

pub(super) fn ensure_json_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }

    match value {
        Value::Object(map) => map,
        _ => unreachable!("json payload should have been normalized to an object"),
    }
}

pub(super) fn insert_optional_path(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<&PathBuf>,
) {
    if let Some(value) = value {
        object.insert(key.to_owned(), Value::String(value.to_string_lossy().into_owned()));
    }
}

pub(super) fn json_property_or_null(value: &Value, key: &str) -> Value {
    value.as_object().and_then(|map| map.get(key)).cloned().unwrap_or(Value::Null)
}

fn value_is_present(value: &Value, key: &str) -> bool {
    value.as_object().and_then(|map| map.get(key)).is_some_and(|value| !value.is_null())
}

fn humanize_artifact_label(id: &str) -> String {
    match id {
        "model" => "Model".into(),
        "diffusion_model" => "Diffusion Model".into(),
        "vae" => "VAE".into(),
        "taesd" => "TAESD".into(),
        "clip_l" => "CLIP L".into(),
        "clip_g" => "CLIP G".into(),
        "t5xxl" => "T5 XXL".into(),
        other => other.replace('_', " "),
    }
}
