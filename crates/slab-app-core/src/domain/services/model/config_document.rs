use std::path::PathBuf;

use serde_json::{Map, Value};
use slab_types::{I18nMessageRef, I18nPayload, ModelSource, ServerI18nKey};

use crate::domain::models::{
    ModelConfigFieldScope, ModelConfigFieldView, ModelConfigOrigin, ModelConfigPresetOption,
    ModelConfigSectionView, ModelConfigSelectionView, ModelConfigSourceArtifact,
    ModelConfigSourceSummary, ModelConfigValueType, ModelConfigVariantOption, ModelPackSelection,
    UnifiedModel,
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
    let path = path.into();
    let label = label.into();
    let i18n = model_config_field_i18n(&path, description_md.as_deref());
    ModelConfigFieldView {
        path,
        scope,
        label,
        description_md,
        i18n,
        value_type,
        effective_value,
        origin,
        editable: false,
        locked: true,
        json_schema: None,
    }
}

pub(super) fn build_model_config_section(
    id: impl Into<String>,
    label: impl Into<String>,
    description_md: Option<String>,
    fields: Vec<ModelConfigFieldView>,
) -> ModelConfigSectionView {
    let id = id.into();
    let label = label.into();
    let i18n = model_config_section_i18n(&id, description_md.as_deref());
    ModelConfigSectionView { id, label, description_md, i18n, fields }
}

fn model_config_section_i18n(id: &str, description_md: Option<&str>) -> Option<I18nPayload> {
    let (label, description) = match id {
        "summary" => (
            Some(ServerI18nKey::ModelConfigSectionSummaryLabel),
            Some(ServerI18nKey::ModelConfigSectionSummaryDescription),
        ),
        "source" => (
            Some(ServerI18nKey::ModelConfigSectionSourceLabel),
            Some(ServerI18nKey::ModelConfigSectionSourceDescription),
        ),
        "load" if description_md.is_some_and(|value| value.contains("does not expose")) => (
            Some(ServerI18nKey::ModelConfigSectionLoadLabel),
            Some(ServerI18nKey::ModelConfigSectionLoadNonRuntimeDescription),
        ),
        "load" => (
            Some(ServerI18nKey::ModelConfigSectionLoadLabel),
            Some(ServerI18nKey::ModelConfigSectionLoadDescription),
        ),
        "inference" => (
            Some(ServerI18nKey::ModelConfigSectionInferenceLabel),
            Some(ServerI18nKey::ModelConfigSectionInferenceDescription),
        ),
        "advanced" if description_md.is_some_and(|value| value.contains("non-runtime")) => (
            Some(ServerI18nKey::ModelConfigSectionAdvancedLabel),
            Some(ServerI18nKey::ModelConfigSectionAdvancedNonRuntimeDescription),
        ),
        "advanced" => (
            Some(ServerI18nKey::ModelConfigSectionAdvancedLabel),
            Some(ServerI18nKey::ModelConfigSectionAdvancedDescription),
        ),
        _ => return None,
    };
    Some(metadata_i18n(label, description))
}

fn model_config_field_i18n(path: &str, description_md: Option<&str>) -> Option<I18nPayload> {
    let (label, description) = match path {
        "model.id" => (
            Some(ServerI18nKey::ModelConfigFieldModelIdLabel),
            Some(ServerI18nKey::ModelConfigFieldModelIdDescription),
        ),
        "model.display_name" => (
            Some(ServerI18nKey::ModelConfigFieldDisplayNameLabel),
            Some(ServerI18nKey::ModelConfigFieldDisplayNameDescription),
        ),
        "model.backend"
            if description_md.is_some_and(|value| value.contains("runtime backend")) =>
        {
            (
                Some(ServerI18nKey::ModelConfigFieldBackendLabel),
                Some(ServerI18nKey::ModelConfigFieldBackendRuntimeDescription),
            )
        }
        "model.backend" => (
            Some(ServerI18nKey::ModelConfigFieldBackendLabel),
            Some(ServerI18nKey::ModelConfigFieldBackendProductDescription),
        ),
        "model.status" => (
            Some(ServerI18nKey::ModelConfigFieldCatalogStatusLabel),
            Some(ServerI18nKey::ModelConfigFieldCatalogStatusDescription),
        ),
        "model.capabilities" => (
            Some(ServerI18nKey::ModelConfigFieldCapabilitiesLabel),
            Some(ServerI18nKey::ModelConfigFieldCapabilitiesDescription),
        ),
        "source.kind" => (
            Some(ServerI18nKey::ModelConfigFieldSourceKindLabel),
            Some(ServerI18nKey::ModelConfigFieldSourceKindDescription),
        ),
        "source.repo_id" => (
            Some(ServerI18nKey::ModelConfigFieldRepoIdLabel),
            Some(ServerI18nKey::ModelConfigFieldRepoIdDescription),
        ),
        "source.filename" => (
            Some(ServerI18nKey::ModelConfigFieldPrimaryArtifactLabel),
            Some(ServerI18nKey::ModelConfigFieldPrimaryArtifactDescription),
        ),
        "source.local_path" => (
            Some(ServerI18nKey::ModelConfigFieldLocalPathLabel),
            Some(ServerI18nKey::ModelConfigFieldLocalPathDescription),
        ),
        _ if path.starts_with("source.artifacts.") => {
            (None, Some(ServerI18nKey::ModelConfigFieldArtifactPathDescription))
        }
        "inference.temperature" => (
            Some(ServerI18nKey::ModelConfigFieldTemperatureLabel),
            Some(ServerI18nKey::ModelConfigFieldTemperatureDescription),
        ),
        "inference.top_p" => (
            Some(ServerI18nKey::ModelConfigFieldTopPLabel),
            Some(ServerI18nKey::ModelConfigFieldTopPDescription),
        ),
        "load.num_workers" => (
            Some(ServerI18nKey::ModelConfigFieldWorkersLabel),
            Some(ServerI18nKey::ModelConfigFieldWorkersDescription),
        ),
        "load.context_length" => (
            Some(ServerI18nKey::ModelConfigFieldContextLengthLabel),
            Some(ServerI18nKey::ModelConfigFieldContextLengthDescription),
        ),
        "load.chat_template" => (
            Some(ServerI18nKey::ModelConfigFieldChatTemplateLabel),
            Some(ServerI18nKey::ModelConfigFieldChatTemplateDescription),
        ),
        "load.gbnf" => (
            Some(ServerI18nKey::ModelConfigFieldGbnfLabel),
            Some(ServerI18nKey::ModelConfigFieldGbnfDescription),
        ),
        "load.diffusion_model_path"
        | "load.vae_path"
        | "load.taesd_path"
        | "load.clip_l_path"
        | "load.clip_g_path"
        | "load.t5xxl_path" => (
            Some(ServerI18nKey::ModelConfigFieldDiffusionAssetLabel),
            Some(ServerI18nKey::ModelConfigFieldDiffusionAssetDescription),
        ),
        "load.flash_attn" => (
            Some(ServerI18nKey::ModelConfigFieldFlashAttentionLabel),
            Some(ServerI18nKey::ModelConfigFieldDiffusionPerformanceDescription),
        ),
        "load.offload_params_to_cpu" => (
            Some(ServerI18nKey::ModelConfigFieldOffloadParamsToCpuLabel),
            Some(ServerI18nKey::ModelConfigFieldDiffusionPerformanceDescription),
        ),
        "load.vae_device" => (
            Some(ServerI18nKey::ModelConfigFieldVaeDeviceLabel),
            Some(ServerI18nKey::ModelConfigFieldDiffusionDeviceDescription),
        ),
        "load.clip_device" => (
            Some(ServerI18nKey::ModelConfigFieldClipDeviceLabel),
            Some(ServerI18nKey::ModelConfigFieldDiffusionDeviceDescription),
        ),
        "load.runtime_load_supported" => (
            Some(ServerI18nKey::ModelConfigFieldRuntimeLoadSupportedLabel),
            Some(ServerI18nKey::ModelConfigFieldRuntimeLoadSupportedDescription),
        ),
        "advanced.non_runtime_projection" => (
            Some(ServerI18nKey::ModelConfigFieldNonRuntimeProjectionLabel),
            Some(ServerI18nKey::ModelConfigFieldNonRuntimeProjectionDescription),
        ),
        "advanced.resolved_load_spec" => (
            Some(ServerI18nKey::ModelConfigFieldResolvedLoadJsonLabel),
            Some(ServerI18nKey::ModelConfigFieldResolvedLoadJsonDescription),
        ),
        "advanced.resolved_inference_spec"
            if description_md.is_some_and(|value| value.contains("selected pack preset")) =>
        {
            (
                Some(ServerI18nKey::ModelConfigFieldResolvedInferenceJsonLabel),
                Some(ServerI18nKey::ModelConfigFieldResolvedInferenceJsonNonRuntimeDescription),
            )
        }
        "advanced.resolved_inference_spec" => (
            Some(ServerI18nKey::ModelConfigFieldResolvedInferenceJsonLabel),
            Some(ServerI18nKey::ModelConfigFieldResolvedInferenceJsonDescription),
        ),
        _ => return None,
    };
    Some(metadata_i18n(label, description))
}

fn metadata_i18n(label: Option<ServerI18nKey>, description: Option<ServerI18nKey>) -> I18nPayload {
    let mut payload = I18nPayload::new();
    if let Some(key) = label {
        payload.insert("label", I18nMessageRef::new(key));
    }
    if let Some(key) = description {
        payload.insert("description_md", I18nMessageRef::new(key));
    }
    payload
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use slab_types::{ModelSource, ServerI18nKey};

    use super::{
        build_model_config_source_fields, build_model_config_source_summary,
        humanize_artifact_label,
    };
    use crate::domain::models::{ModelConfigFieldScope, ModelConfigOrigin, ModelConfigValueType};

    #[test]
    fn hugging_face_source_summary_prefers_named_model_artifact() {
        let source = ModelSource::HuggingFace {
            repo_id: "Qwen/Qwen3".to_owned(),
            revision: Some("main".to_owned()),
            files: BTreeMap::from([
                ("tokenizer".to_owned(), PathBuf::from("tokenizer.json")),
                ("model".to_owned(), PathBuf::from("qwen.gguf")),
                ("clip_l".to_owned(), PathBuf::from("clip-l.safetensors")),
            ]),
        };

        let summary = build_model_config_source_summary(&source);

        assert_eq!(summary.source_kind, "hugging_face");
        assert_eq!(summary.repo_id.as_deref(), Some("Qwen/Qwen3"));
        assert_eq!(summary.filename.as_deref(), Some("qwen.gguf"));
        assert_eq!(summary.local_path, None);
        assert!(
            summary
                .artifacts
                .iter()
                .any(|artifact| artifact.id == "clip_l" && artifact.label == "CLIP L")
        );
    }

    #[test]
    fn local_artifacts_without_model_use_first_artifact_as_primary_path() {
        let source = ModelSource::LocalArtifacts {
            files: BTreeMap::from([
                ("vae".to_owned(), PathBuf::from("vae.safetensors")),
                ("clip_l".to_owned(), PathBuf::from("clip-l.safetensors")),
            ]),
        };

        let summary = build_model_config_source_summary(&source);

        assert_eq!(summary.source_kind, "local_artifacts");
        assert_eq!(summary.filename, None);
        assert_eq!(summary.local_path.as_deref(), Some("clip-l.safetensors"));
        assert_eq!(summary.artifacts.len(), 2);
    }

    #[test]
    fn source_fields_only_emit_optional_values_that_exist() {
        let source = ModelSource::LocalPath { path: PathBuf::from("/models/qwen.gguf") };
        let summary = build_model_config_source_summary(&source);

        let fields = build_model_config_source_fields(&summary, ModelConfigOrigin::SelectedVariant);
        let paths = fields.iter().map(|field| field.path.as_str()).collect::<Vec<_>>();

        assert_eq!(paths, vec!["source.kind", "source.local_path", "source.artifacts.model"]);
        assert!(fields.iter().all(|field| field.origin == ModelConfigOrigin::SelectedVariant));
    }

    #[test]
    fn model_config_fields_include_metadata_i18n_when_known() {
        let field = super::build_model_config_field(
            "model.id",
            ModelConfigFieldScope::Summary,
            "Model ID",
            Some("Catalog identifier projected from the pack manifest.".into()),
            ModelConfigValueType::String,
            serde_json::Value::String("model-1".into()),
            ModelConfigOrigin::PackManifest,
        );

        let i18n = field.i18n.expect("field i18n");
        assert_eq!(
            i18n.0.get("label").map(|message| message.key),
            Some(ServerI18nKey::ModelConfigFieldModelIdLabel)
        );
        assert_eq!(
            i18n.0.get("description_md").map(|message| message.key),
            Some(ServerI18nKey::ModelConfigFieldModelIdDescription)
        );
    }

    #[test]
    fn model_config_sections_include_metadata_i18n_when_known() {
        let section = super::build_model_config_section(
            "load",
            "Load",
            Some("Effective runtime load parameters.".into()),
            Vec::new(),
        );

        let i18n = section.i18n.expect("section i18n");
        assert_eq!(
            i18n.0.get("label").map(|message| message.key),
            Some(ServerI18nKey::ModelConfigSectionLoadLabel)
        );
        assert_eq!(
            i18n.0.get("description_md").map(|message| message.key),
            Some(ServerI18nKey::ModelConfigSectionLoadDescription)
        );
    }

    #[test]
    fn unknown_artifact_labels_are_humanized_without_title_casing() {
        assert_eq!(humanize_artifact_label("audio_encoder"), "audio encoder");
    }
}
