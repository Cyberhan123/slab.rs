mod catalog;
mod download;
mod pack;
mod runtime;

pub(crate) use catalog::list_chat_models_from_state;

use std::path::PathBuf;

use serde_json::{Map, Value};
use slab_types::{ModelSource, RuntimeBackendId};

use crate::context::{ModelState, WorkerState};
use crate::domain::models::{
    CreateModelCommand, ModelConfigDocument, ModelConfigFieldScope, ModelConfigFieldView,
    ModelConfigOrigin, ModelConfigPresetOption, ModelConfigSectionView, ModelConfigSelectionView,
    ModelConfigSourceArtifact, ModelConfigSourceSummary, ModelConfigValueType,
    ModelConfigVariantOption, ModelPackSelection, UnifiedModel,
};
use crate::error::AppCoreError;
use crate::infra::model_packs;

#[derive(Clone)]
pub struct ModelService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl ModelService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self { model_state, worker_state }
    }

    pub async fn get_model_config_document(
        &self,
        id: &str,
    ) -> Result<ModelConfigDocument, AppCoreError> {
        let model = self.get_model(id).await?;
        runtime::resolve_local_backend_from_model(&model)?;

        let context = self.load_model_pack_context(id)?;
        let selection = self
            .resolve_model_pack_selection(id, &context.resolved, context.persisted.as_ref(), true)
            .await?;
        let command =
            pack::build_model_command_from_pack_context(&context, &selection.selected_preset)?;
        let selection_view = build_model_config_selection_view(
            &context.resolved,
            &selection.explicit_selection,
            &selection.effective_selection,
        );
        let (sections, source_summary, resolved_load_spec, resolved_inference_spec) = match context
            .resolved
            .compile_runtime_bridge(&selection.selected_preset)
        {
            Ok(mut bridge) => {
                pack::apply_materialized_source_to_bridge(
                    &mut bridge,
                    context.persisted.as_ref(),
                    selection.selected_preset.variant.effective_sources.first(),
                );
                let source_summary = build_model_config_source_summary(&bridge.model_spec.source);
                let resolved_load_spec = self
                    .build_model_config_load_json(
                        bridge.backend,
                        &command,
                        &bridge,
                        selection.selected_preset.effective_load_config.as_ref(),
                    )
                    .await?;
                let resolved_inference_spec = Value::Object(
                    bridge.inference_defaults.clone().into_iter().collect::<Map<String, Value>>(),
                );
                let sections = self.build_model_config_sections(
                    &model,
                    &command,
                    &context.resolved,
                    &selection.selected_preset,
                    &bridge,
                    &source_summary,
                    &resolved_load_spec,
                    &resolved_inference_spec,
                )?;
                (sections, source_summary, resolved_load_spec, resolved_inference_spec)
            }
            Err(slab_model_pack::ModelPackError::MissingRuntimeCapability)
                if !pack::pack_has_runtime_execution_capability(&context.resolved.manifest) =>
            {
                let source = pack::materialized_model_source(
                    &pack::resolve_pack_model_source(
                        &context.resolved,
                        &selection.selected_preset,
                        "failed to resolve selected pack preset source for config document",
                    )?,
                    context.persisted.as_ref(),
                    selection.selected_preset.variant.effective_sources.first(),
                );
                let source_summary = build_model_config_source_summary(&source);
                let resolved_inference_spec = selection
                    .selected_preset
                    .effective_inference_config
                    .as_ref()
                    .map(|config| config.payload.clone())
                    .unwrap_or_else(|| Value::Object(Map::new()));
                let resolved_load_spec = Value::Object(Map::new());
                let sections = self.build_product_model_config_sections(
                    &model,
                    &command,
                    &context.resolved,
                    &selection.selected_preset,
                    &source_summary,
                    &resolved_inference_spec,
                )?;
                (sections, source_summary, resolved_load_spec, resolved_inference_spec)
            }
            Err(error) => {
                return Err(AppCoreError::BadRequest(format!(
                    "failed to compile selected pack preset for config document: {error}"
                )));
            }
        };

        Ok(ModelConfigDocument {
            model_summary: model,
            selection: selection_view,
            sections,
            source_summary,
            resolved_load_spec,
            resolved_inference_spec,
            warnings: selection.warnings,
        })
    }

    async fn build_model_config_load_json(
        &self,
        backend_id: RuntimeBackendId,
        command: &CreateModelCommand,
        bridge: &slab_model_pack::ModelPackRuntimeBridge,
        load_config: Option<&slab_model_pack::BackendConfigDocument>,
    ) -> Result<Value, AppCoreError> {
        let mut payload = load_config
            .map(|config| config.payload.clone())
            .unwrap_or_else(|| Value::Object(Map::new()));
        let object = ensure_json_object(&mut payload);
        let display_model_path =
            command.spec.local_path.clone().or_else(|| command.spec.filename.clone()).or_else(
                || {
                    bridge
                        .model_spec
                        .source
                        .primary_path()
                        .map(|path| path.to_string_lossy().into_owned())
                },
            );

        if let Some(model_path) = display_model_path {
            object.insert("model_path".into(), Value::String(model_path));
        }

        let (workers, _) = if let Some(workers) = bridge.load_defaults.num_workers {
            runtime::validate_and_normalize_model_workers(backend_id, workers, "model_pack")?
        } else {
            runtime::resolve_model_workers(&self.model_state, backend_id, None).await?
        };
        object.insert("num_workers".into(), Value::from(workers));

        if backend_id == RuntimeBackendId::GgmlLlama {
            if let Some(context_length) = bridge.load_defaults.context_length {
                object.insert("context_length".into(), Value::from(context_length));
            } else {
                let (context_length, source) =
                    runtime::resolve_llama_context_length(&self.model_state, backend_id).await?;
                if context_length > 0 || source == "settings" {
                    object.insert("context_length".into(), Value::from(context_length));
                }
            }
            if let Some(chat_template) = bridge.load_defaults.chat_template.as_ref() {
                object.insert("chat_template".into(), Value::String(chat_template.clone()));
            }
        }

        if backend_id == RuntimeBackendId::GgmlDiffusion {
            let diffusion = model_packs::merge_diffusion_load_defaults(
                bridge.load_defaults.diffusion.clone(),
                runtime::resolve_diffusion_context_params(&self.model_state, backend_id).await?,
            )
            .unwrap_or_default();

            insert_optional_path(
                object,
                "diffusion_model_path",
                diffusion.diffusion_model_path.as_ref(),
            );
            insert_optional_path(object, "vae_path", diffusion.vae_path.as_ref());
            insert_optional_path(object, "taesd_path", diffusion.taesd_path.as_ref());
            insert_optional_path(object, "clip_l_path", diffusion.clip_l_path.as_ref());
            insert_optional_path(object, "clip_g_path", diffusion.clip_g_path.as_ref());
            insert_optional_path(object, "t5xxl_path", diffusion.t5xxl_path.as_ref());
            object.insert("flash_attn".into(), Value::Bool(diffusion.flash_attn));
            if !diffusion.vae_device.is_empty() {
                object.insert("vae_device".into(), Value::String(diffusion.vae_device));
            }
            if !diffusion.clip_device.is_empty() {
                object.insert("clip_device".into(), Value::String(diffusion.clip_device));
            }
            object.insert(
                "offload_params_to_cpu".into(),
                Value::Bool(diffusion.offload_params_to_cpu),
            );
        }

        Ok(payload)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_model_config_sections(
        &self,
        model: &UnifiedModel,
        command: &CreateModelCommand,
        resolved: &slab_model_pack::ResolvedModelPack,
        selected_preset: &slab_model_pack::ResolvedPreset,
        bridge: &slab_model_pack::ModelPackRuntimeBridge,
        source_summary: &ModelConfigSourceSummary,
        resolved_load_spec: &Value,
        resolved_inference_spec: &Value,
    ) -> Result<Vec<ModelConfigSectionView>, AppCoreError> {
        let source_origin = model_source_origin(selected_preset);
        let summary_fields = vec![
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
                Value::String(command.display_name.clone()),
                ModelConfigOrigin::PackManifest,
            ),
            build_model_config_field(
                "model.backend",
                ModelConfigFieldScope::Summary,
                "Backend",
                Some("Managed runtime backend selected for this pack.".into()),
                ModelConfigValueType::String,
                Value::String(bridge.backend.canonical_id().to_owned()),
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
        ];

        let mut source_fields = vec![build_model_config_field(
            "source.kind",
            ModelConfigFieldScope::Source,
            "Source Kind",
            Some("Where the selected preset resolves its artifacts from.".into()),
            ModelConfigValueType::String,
            Value::String(source_summary.source_kind.clone()),
            source_origin,
        )];
        if let Some(repo_id) = source_summary.repo_id.as_ref() {
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
                format!("source.artifacts.{}", artifact.id),
                ModelConfigFieldScope::Source,
                artifact.label.clone(),
                Some("Resolved artifact path for the selected source.".into()),
                ModelConfigValueType::Path,
                Value::String(artifact.value.clone()),
                source_origin,
            ));
        }

        let mut load_fields = vec![build_model_config_field(
            "load.num_workers",
            ModelConfigFieldScope::Load,
            "Workers",
            Some("Effective worker count used when loading the runtime model.".into()),
            ModelConfigValueType::Integer,
            json_property_or_null(resolved_load_spec, "num_workers"),
            if bridge.load_defaults.num_workers.is_some() {
                ModelConfigOrigin::SelectedBackendConfig
            } else {
                ModelConfigOrigin::PmidFallback
            },
        )];
        match bridge.backend {
            RuntimeBackendId::GgmlLlama => {
                load_fields.push(build_model_config_field(
                    "load.context_length",
                    ModelConfigFieldScope::Load,
                    "Context Length",
                    Some("Effective llama context window length in tokens.".into()),
                    ModelConfigValueType::Integer,
                    json_property_or_null(resolved_load_spec, "context_length"),
                    if resolved.manifest.context_window.is_some() {
                        ModelConfigOrigin::PackManifest
                    } else if bridge.load_defaults.context_length.is_some() {
                        ModelConfigOrigin::SelectedBackendConfig
                    } else {
                        ModelConfigOrigin::PmidFallback
                    },
                ));
                load_fields.push(build_model_config_field(
                    "load.chat_template",
                    ModelConfigFieldScope::Load,
                    "Chat Template",
                    Some("Effective chat template resolved for llama chat formatting.".into()),
                    ModelConfigValueType::String,
                    json_property_or_null(resolved_load_spec, "chat_template"),
                    if bridge.load_defaults.chat_template.is_some() {
                        ModelConfigOrigin::SelectedBackendConfig
                    } else {
                        ModelConfigOrigin::Derived
                    },
                ));
            }
            RuntimeBackendId::GgmlDiffusion => {
                for (path, label) in [
                    ("diffusion_model_path", "Diffusion Model"),
                    ("vae_path", "VAE"),
                    ("taesd_path", "TAESD"),
                    ("clip_l_path", "CLIP L"),
                    ("clip_g_path", "CLIP G"),
                    ("t5xxl_path", "T5 XXL"),
                ] {
                    load_fields.push(build_model_config_field(
                        format!("load.{path}"),
                        ModelConfigFieldScope::Load,
                        label,
                        Some("Effective diffusion asset path passed to runtime load.".into()),
                        ModelConfigValueType::Path,
                        json_property_or_null(resolved_load_spec, path),
                        diffusion_load_origin(bridge, path),
                    ));
                }
                for (path, label) in [
                    ("flash_attn", "Flash Attention"),
                    ("offload_params_to_cpu", "Offload Params To CPU"),
                ] {
                    load_fields.push(build_model_config_field(
                        format!("load.{path}"),
                        ModelConfigFieldScope::Load,
                        label,
                        Some("Effective diffusion runtime performance toggle.".into()),
                        ModelConfigValueType::Boolean,
                        json_property_or_null(resolved_load_spec, path),
                        diffusion_load_origin(bridge, path),
                    ));
                }
                for (path, label) in [("vae_device", "VAE Device"), ("clip_device", "CLIP Device")]
                {
                    load_fields.push(build_model_config_field(
                        format!("load.{path}"),
                        ModelConfigFieldScope::Load,
                        label,
                        Some(
                            "Effective device override for diffusion auxiliary components.".into(),
                        ),
                        ModelConfigValueType::String,
                        json_property_or_null(resolved_load_spec, path),
                        diffusion_load_origin(bridge, path),
                    ));
                }
            }
            _ => {}
        }

        let mut inference_fields = Vec::new();
        if resolved.manifest.runtime_presets.as_ref().and_then(|value| value.temperature).is_some()
            || value_is_present(resolved_inference_spec, "temperature")
        {
            inference_fields.push(build_model_config_field(
                "inference.temperature",
                ModelConfigFieldScope::Inference,
                "Temperature",
                Some("Resolved sampling temperature exposed by the pack.".into()),
                ModelConfigValueType::Number,
                json_property_or_null(resolved_inference_spec, "temperature"),
                if resolved
                    .manifest
                    .runtime_presets
                    .as_ref()
                    .and_then(|value| value.temperature)
                    .is_some()
                {
                    ModelConfigOrigin::PackManifest
                } else {
                    ModelConfigOrigin::SelectedBackendConfig
                },
            ));
        }
        if resolved.manifest.runtime_presets.as_ref().and_then(|value| value.top_p).is_some()
            || value_is_present(resolved_inference_spec, "top_p")
        {
            inference_fields.push(build_model_config_field(
                "inference.top_p",
                ModelConfigFieldScope::Inference,
                "Top P",
                Some("Resolved nucleus sampling value exposed by the pack.".into()),
                ModelConfigValueType::Number,
                json_property_or_null(resolved_inference_spec, "top_p"),
                if resolved
                    .manifest
                    .runtime_presets
                    .as_ref()
                    .and_then(|value| value.top_p)
                    .is_some()
                {
                    ModelConfigOrigin::PackManifest
                } else {
                    ModelConfigOrigin::SelectedBackendConfig
                },
            ));
        }

        let advanced_fields = vec![
            build_model_config_field(
                "advanced.resolved_load_spec",
                ModelConfigFieldScope::Advanced,
                "Resolved Load JSON",
                Some("Full resolved load document after pack selection and PMID fallback.".into()),
                ModelConfigValueType::Json,
                resolved_load_spec.clone(),
                ModelConfigOrigin::Derived,
            ),
            build_model_config_field(
                "advanced.resolved_inference_spec",
                ModelConfigFieldScope::Advanced,
                "Resolved Inference JSON",
                Some("Full resolved inference document after pack selection.".into()),
                ModelConfigValueType::Json,
                resolved_inference_spec.clone(),
                ModelConfigOrigin::Derived,
            ),
        ];

        Ok(vec![
            ModelConfigSectionView {
                id: "summary".into(),
                label: "Summary".into(),
                description_md: Some("Pack-backed catalog summary for the selected model.".into()),
                fields: summary_fields,
            },
            ModelConfigSectionView {
                id: "source".into(),
                label: "Source / Artifacts".into(),
                description_md: Some(
                    "Resolved source and artifacts for the active selection.".into(),
                ),
                fields: source_fields,
            },
            ModelConfigSectionView {
                id: "load".into(),
                label: "Load".into(),
                description_md: Some("Effective runtime load parameters.".into()),
                fields: load_fields,
            },
            ModelConfigSectionView {
                id: "inference".into(),
                label: "Inference".into(),
                description_md: Some("Resolved inference defaults from the pack.".into()),
                fields: inference_fields,
            },
            ModelConfigSectionView {
                id: "advanced".into(),
                label: "Advanced".into(),
                description_md: Some(
                    "Fallback JSON for fields not yet promoted into the canonical catalog.".into(),
                ),
                fields: advanced_fields,
            },
        ])
    }

    fn build_product_model_config_sections(
        &self,
        model: &UnifiedModel,
        command: &CreateModelCommand,
        resolved: &slab_model_pack::ResolvedModelPack,
        selected_preset: &slab_model_pack::ResolvedPreset,
        source_summary: &ModelConfigSourceSummary,
        resolved_inference_spec: &Value,
    ) -> Result<Vec<ModelConfigSectionView>, AppCoreError> {
        let source_origin = model_source_origin(selected_preset);
        let backend_label = command
            .backend_id
            .map(|backend_id| backend_id.to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        let summary_fields = vec![
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
                Value::String(command.display_name.clone()),
                ModelConfigOrigin::PackManifest,
            ),
            build_model_config_field(
                "model.backend",
                ModelConfigFieldScope::Summary,
                "Backend",
                Some("Managed backend used for catalog projection and downloads.".into()),
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
        ];

        let mut source_fields = vec![build_model_config_field(
            "source.kind",
            ModelConfigFieldScope::Source,
            "Source Kind",
            Some("Where the selected preset resolves its artifacts from.".into()),
            ModelConfigValueType::String,
            Value::String(source_summary.source_kind.clone()),
            source_origin,
        )];
        if let Some(repo_id) = source_summary.repo_id.as_ref() {
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
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
            source_fields.push(build_model_config_field(
                format!("source.artifacts.{}", artifact.id),
                ModelConfigFieldScope::Source,
                artifact.label.clone(),
                Some("Resolved artifact path for the selected source.".into()),
                ModelConfigValueType::Path,
                Value::String(artifact.value.clone()),
                source_origin,
            ));
        }

        let load_fields = vec![build_model_config_field(
            "load.runtime_load_supported",
            ModelConfigFieldScope::Load,
            "Runtime Load Supported",
            Some("Whether this pack resolves to a runtime-loadable model target.".into()),
            ModelConfigValueType::Boolean,
            Value::Bool(false),
            ModelConfigOrigin::Derived,
        )];

        let mut inference_fields = Vec::new();
        if resolved.manifest.runtime_presets.as_ref().and_then(|value| value.temperature).is_some()
            || value_is_present(resolved_inference_spec, "temperature")
        {
            inference_fields.push(build_model_config_field(
                "inference.temperature",
                ModelConfigFieldScope::Inference,
                "Temperature",
                Some("Resolved sampling temperature exposed by the pack.".into()),
                ModelConfigValueType::Number,
                json_property_or_null(resolved_inference_spec, "temperature"),
                if resolved
                    .manifest
                    .runtime_presets
                    .as_ref()
                    .and_then(|value| value.temperature)
                    .is_some()
                {
                    ModelConfigOrigin::PackManifest
                } else {
                    ModelConfigOrigin::SelectedBackendConfig
                },
            ));
        }
        if resolved.manifest.runtime_presets.as_ref().and_then(|value| value.top_p).is_some()
            || value_is_present(resolved_inference_spec, "top_p")
        {
            inference_fields.push(build_model_config_field(
                "inference.top_p",
                ModelConfigFieldScope::Inference,
                "Top P",
                Some("Resolved nucleus sampling value exposed by the pack.".into()),
                ModelConfigValueType::Number,
                json_property_or_null(resolved_inference_spec, "top_p"),
                if resolved
                    .manifest
                    .runtime_presets
                    .as_ref()
                    .and_then(|value| value.top_p)
                    .is_some()
                {
                    ModelConfigOrigin::PackManifest
                } else {
                    ModelConfigOrigin::SelectedBackendConfig
                },
            ));
        }

        let advanced_fields = vec![
            build_model_config_field(
                "advanced.non_runtime_projection",
                ModelConfigFieldScope::Advanced,
                "Non-Runtime Projection",
                Some(
                    "This pack is cataloged for download and product usage only, without a runtime bridge."
                        .into(),
                ),
                ModelConfigValueType::Boolean,
                Value::Bool(true),
                ModelConfigOrigin::Derived,
            ),
            build_model_config_field(
                "advanced.resolved_inference_spec",
                ModelConfigFieldScope::Advanced,
                "Resolved Inference JSON",
                Some("Resolved inference defaults from the selected pack preset.".into()),
                ModelConfigValueType::Json,
                resolved_inference_spec.clone(),
                ModelConfigOrigin::Derived,
            ),
        ];

        Ok(vec![
            ModelConfigSectionView {
                id: "summary".into(),
                label: "Summary".into(),
                description_md: Some("Pack-backed catalog summary for the selected model.".into()),
                fields: summary_fields,
            },
            ModelConfigSectionView {
                id: "source".into(),
                label: "Source / Artifacts".into(),
                description_md: Some("Resolved source and artifacts for the active selection.".into()),
                fields: source_fields,
            },
            ModelConfigSectionView {
                id: "load".into(),
                label: "Load".into(),
                description_md: Some(
                    "This pack does not expose a runtime-execution capability, so runtime load settings are unavailable."
                        .into(),
                ),
                fields: load_fields,
            },
            ModelConfigSectionView {
                id: "inference".into(),
                label: "Inference".into(),
                description_md: Some("Resolved inference defaults from the pack.".into()),
                fields: inference_fields,
            },
            ModelConfigSectionView {
                id: "advanced".into(),
                label: "Advanced".into(),
                description_md: Some("Additional metadata for the selected non-runtime pack.".into()),
                fields: advanced_fields,
            },
        ])
    }
}

fn build_model_config_selection_view(
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

fn build_model_config_source_summary(source: &ModelSource) -> ModelConfigSourceSummary {
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

fn build_model_config_field(
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

fn model_source_origin(selected_preset: &slab_model_pack::ResolvedPreset) -> ModelConfigOrigin {
    if !selected_preset.variant.document.sources.is_empty()
        || !selected_preset.variant.components.is_empty()
    {
        ModelConfigOrigin::SelectedVariant
    } else {
        ModelConfigOrigin::PackManifest
    }
}

fn diffusion_load_origin(
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

fn ensure_json_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }

    match value {
        Value::Object(map) => map,
        _ => unreachable!("json payload should have been normalized to an object"),
    }
}

fn insert_optional_path(object: &mut Map<String, Value>, key: &str, value: Option<&PathBuf>) {
    if let Some(value) = value {
        object.insert(key.to_owned(), Value::String(value.to_string_lossy().into_owned()));
    }
}

fn json_property_or_null(value: &Value, key: &str) -> Value {
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

    use chrono::Utc;
    use slab_hub::HubErrorKind;
    use slab_types::{Capability, DriverHints, ModelFamily, RuntimeBackendId};

    use crate::domain::models::{
        ChatModelSource, ModelSpec, RuntimePresets, UnifiedModel, UnifiedModelKind,
        UnifiedModelStatus, default_model_capabilities,
    };
    use crate::error::AppCoreError;

    use super::catalog::{
        build_cloud_chat_model_option, build_local_chat_model_option, canonicalize_model_spec,
        canonicalize_runtime_presets, map_hub_client_error, normalize_required_text,
    };
    use super::pack::build_local_model_command_from_pack_preset;
    use super::runtime::{map_grpc_model_error, validate_and_normalize_model_workers};

    #[test]
    fn cloud_models_require_provider_reference() {
        let error = canonicalize_model_spec(UnifiedModelKind::Cloud, None, ModelSpec::default())
            .expect_err("missing cloud fields");

        assert!(
            error.to_string().contains(
                "cloud models must set spec.provider_id to a configured providers.registry entry"
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn cloud_models_require_remote_model_id() {
        let error = canonicalize_model_spec(
            UnifiedModelKind::Cloud,
            None,
            ModelSpec { provider_id: Some("openai-main".into()), ..ModelSpec::default() },
        )
        .expect_err("missing remote_model_id");

        assert!(
            error.to_string().contains("cloud models must set spec.remote_model_id"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn cloud_models_trim_provider_and_remote_model() {
        let (_, spec) = canonicalize_model_spec(
            UnifiedModelKind::Cloud,
            None,
            ModelSpec {
                provider_id: Some(" openai-main ".into()),
                remote_model_id: Some(" gpt-4.1-mini ".into()),
                ..ModelSpec::default()
            },
        )
        .expect("cloud spec");

        assert_eq!(spec.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
    }

    #[test]
    fn cloud_models_clear_local_only_fields() {
        let (_, spec) = canonicalize_model_spec(
            UnifiedModelKind::Cloud,
            None,
            ModelSpec {
                provider_id: Some("openai-main".into()),
                remote_model_id: Some("gpt-4.1-mini".into()),
                repo_id: Some("Qwen/Qwen3-8B-GGUF".into()),
                hub_provider: Some("hf".into()),
                filename: Some("qwen3-8b.gguf".into()),
                local_path: Some("C:/models/qwen3-8b.gguf".into()),
                chat_template: Some("chatml".into()),
                ..ModelSpec::default()
            },
        )
        .expect("cloud spec");

        assert!(spec.repo_id.is_none());
        assert!(spec.hub_provider.is_none());
        assert!(spec.filename.is_none());
        assert!(spec.local_path.is_none());
        assert!(spec.chat_template.is_none());
    }

    #[test]
    fn local_models_require_backend_id() {
        let error = canonicalize_model_spec(UnifiedModelKind::Local, None, ModelSpec::default())
            .expect_err("missing backend_id");

        assert!(
            error.to_string().contains("local models must set backend_id"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn local_models_clear_cloud_only_fields_and_canonicalize_backend_id() {
        let (backend_id, spec) = canonicalize_model_spec(
            UnifiedModelKind::Local,
            Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
            ModelSpec {
                provider_id: Some("openai-main".into()),
                remote_model_id: Some("gpt-4.1-mini".into()),
                ..ModelSpec::default()
            },
        )
        .expect("local spec");

        assert_eq!(backend_id, Some(crate::domain::models::ManagedModelBackendId::GgmlLlama));
        assert!(spec.provider_id.is_none());
        assert!(spec.remote_model_id.is_none());
    }

    #[test]
    fn local_models_canonicalize_explicit_hub_provider() {
        let (_, spec) = canonicalize_model_spec(
            UnifiedModelKind::Local,
            Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
            ModelSpec { hub_provider: Some(" hf ".into()), ..ModelSpec::default() },
        )
        .expect("local spec");

        assert_eq!(spec.hub_provider.as_deref(), Some("hf_hub"));
    }

    #[test]
    fn local_models_reject_unknown_hub_provider() {
        let error = canonicalize_model_spec(
            UnifiedModelKind::Local,
            Some(crate::domain::models::ManagedModelBackendId::GgmlLlama),
            ModelSpec { hub_provider: Some("unknown".into()), ..ModelSpec::default() },
        )
        .expect_err("invalid hub provider");

        assert!(
            error.to_string().contains("unsupported hub provider"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn hub_invalid_repo_errors_map_to_bad_request() {
        let error = map_hub_client_error(
            "hub file listing failed",
            HubErrorKind::InvalidRepoId,
            "repo_id is invalid".to_owned(),
        );

        assert!(
            matches!(error, AppCoreError::BadRequest(message) if message.contains("repo_id is invalid"))
        );
    }

    #[test]
    fn hub_network_errors_map_to_backend_not_ready() {
        let error = map_hub_client_error(
            "hub file listing failed",
            HubErrorKind::NetworkUnavailable,
            "network unreachable".to_owned(),
        );

        assert!(
            matches!(error, AppCoreError::BackendNotReady(message) if message.contains("network unreachable"))
        );
    }

    #[test]
    fn local_chat_picker_only_includes_llama_models() {
        let whisper = make_model(
            UnifiedModelKind::Local,
            Some("ggml.whisper"),
            None,
            None,
            UnifiedModelStatus::Ready,
            Some("C:/models/whisper.bin"),
        );
        assert!(build_local_chat_model_option(&whisper).is_none());

        let llama = make_model(
            UnifiedModelKind::Local,
            Some("ggml.llama"),
            None,
            None,
            UnifiedModelStatus::Downloading,
            None,
        );
        let option = build_local_chat_model_option(&llama).expect("llama option");

        assert_eq!(option.source, ChatModelSource::Local);
        assert_eq!(
            option.backend_id,
            Some(crate::domain::models::ManagedModelBackendId::GgmlLlama)
        );
        assert!(option.pending);
        assert!(!option.downloaded);
    }

    #[test]
    fn cloud_chat_picker_requires_known_provider() {
        let model = make_model(
            UnifiedModelKind::Cloud,
            None,
            Some("openai-main"),
            Some("gpt-4.1-mini"),
            UnifiedModelStatus::Ready,
            None,
        );

        assert!(build_cloud_chat_model_option(&BTreeMap::new(), &model).is_none());

        let mut providers = BTreeMap::new();
        providers.insert(
            "openai-main".to_owned(),
            slab_types::settings::CloudProviderConfig {
                id: "openai-main".to_owned(),
                name: "OpenAI".to_owned(),
                api_base: "https://api.openai.com/v1".to_owned(),
                api_key: None,
                api_key_env: None,
            },
        );

        let option = build_cloud_chat_model_option(&providers, &model).expect("cloud option");
        assert_eq!(option.source, ChatModelSource::Cloud);
        assert_eq!(option.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(option.provider_name.as_deref(), Some("OpenAI"));
    }

    #[test]
    fn empty_runtime_presets_are_dropped() {
        let presets =
            canonicalize_runtime_presets(Some(RuntimePresets { temperature: None, top_p: None }));

        assert!(presets.is_none());
    }

    #[test]
    fn required_text_fields_are_trimmed() {
        let value = normalize_required_text("  model-id  ".into(), "id").expect("trimmed value");

        assert_eq!(value, "model-id");
    }

    #[test]
    fn transient_transport_errors_map_to_backend_not_ready() {
        let error = anyhow::Error::new(tonic::Status::unknown(
            "transport error: broken pipe while reconnecting runtime",
        ));

        let mapped = map_grpc_model_error("load_model", error);
        match mapped {
            AppCoreError::BackendNotReady(detail) => {
                assert!(detail.contains("transport error"));
            }
            other => panic!("expected BackendNotReady, got {other:?}"),
        }
    }

    #[test]
    fn diffusion_workers_are_clamped_to_one() {
        let (workers, source) =
            validate_and_normalize_model_workers(RuntimeBackendId::GgmlDiffusion, 4, "settings")
                .expect("diffusion worker count should normalize");

        assert_eq!(workers, 1);
        assert_eq!(source, "settings");
    }

    #[test]
    fn non_diffusion_workers_keep_requested_count() {
        let (workers, source) =
            validate_and_normalize_model_workers(RuntimeBackendId::GgmlWhisper, 3, "request")
                .expect("whisper worker count should normalize");

        assert_eq!(workers, 3);
        assert_eq!(source, "request");
    }

    #[test]
    fn product_only_vad_pack_projects_into_local_catalog_model() {
        let manifest = slab_model_pack::ModelPackManifest {
            version: 2,
            id: "whisper-vad".into(),
            label: "whisper-vad".into(),
            status: None,
            family: ModelFamily::Whisper,
            capabilities: vec![Capability::AudioVad],
            backend_hints: DriverHints {
                prefer_drivers: vec!["ggml.whisper".into()],
                avoid_drivers: Vec::new(),
                require_streaming: false,
            },
            context_window: None,
            pricing: None,
            runtime_presets: None,
            metadata: BTreeMap::new(),
            sources: vec![slab_model_pack::PackSourceCandidate::new(
                slab_model_pack::PackSource::HuggingFace {
                    repo_id: "ggml-org/whisper-vad".into(),
                    revision: None,
                    files: vec![slab_model_pack::PackSourceFile {
                        id: "model".into(),
                        label: None,
                        description: None,
                        path: "ggml-silero-v6.2.0.bin".into(),
                    }],
                },
            )],
            components: Vec::new(),
            variants: Vec::new(),
            adapters: Vec::new(),
            presets: Vec::new(),
            default_preset: Some("default".into()),
            footprint: Default::default(),
        };
        let preset = slab_model_pack::ResolvedPreset {
            document: slab_model_pack::PresetDocument {
                id: "default".into(),
                label: "Default".into(),
                variant_id: None,
                description: None,
                adapter_ids: Vec::new(),
                load_config: None,
                inference_config: None,
                footprint: Default::default(),
                metadata: BTreeMap::new(),
            },
            variant: slab_model_pack::ResolvedVariant {
                document: slab_model_pack::VariantDocument {
                    id: String::new(),
                    label: "Original Model".into(),
                    description: None,
                    sources: Vec::new(),
                    component_ids: Vec::new(),
                    load_config: None,
                    inference_config: None,
                    metadata: BTreeMap::new(),
                },
                effective_sources: manifest.sources.clone(),
                components: BTreeMap::new(),
                load_config: None,
                inference_config: None,
            },
            adapters: BTreeMap::new(),
            effective_load_config: None,
            effective_inference_config: None,
        };
        let mut presets = BTreeMap::new();
        presets.insert("default".into(), preset.clone());
        let resolved = slab_model_pack::ResolvedModelPack {
            manifest: manifest.clone(),
            components: BTreeMap::new(),
            adapters: BTreeMap::new(),
            variants: BTreeMap::new(),
            presets,
            default_preset_id: Some("default".into()),
        };

        assert!(
            matches!(
                resolved.compile_runtime_bridge(&preset),
                Err(slab_model_pack::ModelPackError::MissingRuntimeCapability)
            ),
            "pure VAD packs should still skip runtime bridge compilation"
        );

        let command = build_local_model_command_from_pack_preset(&manifest, &resolved, &preset)
            .expect("project vad pack into local catalog model");

        assert_eq!(
            command.backend_id,
            Some(crate::domain::models::ManagedModelBackendId::GgmlWhisper)
        );
        assert_eq!(command.capabilities, Some(vec![Capability::AudioVad]));
        assert_eq!(command.status, Some(UnifiedModelStatus::NotDownloaded));
        assert_eq!(command.spec.repo_id.as_deref(), Some("ggml-org/whisper-vad"));
        assert_eq!(command.spec.hub_provider.as_deref(), Some("hf_hub"));
        assert_eq!(command.spec.filename.as_deref(), Some("ggml-silero-v6.2.0.bin"));
        assert!(command.spec.local_path.is_none());
    }

    #[test]
    fn zero_workers_are_rejected() {
        let error =
            validate_and_normalize_model_workers(RuntimeBackendId::GgmlDiffusion, 0, "request")
                .expect_err("zero workers should fail validation");

        assert!(
            matches!(error, AppCoreError::BadRequest(message) if message.contains("at least 1"))
        );
    }

    fn make_model(
        kind: UnifiedModelKind,
        backend_id: Option<&str>,
        provider_id: Option<&str>,
        remote_model_id: Option<&str>,
        status: UnifiedModelStatus,
        local_path: Option<&str>,
    ) -> UnifiedModel {
        let backend_id = backend_id.map(|value| value.parse().expect("managed model backend id"));

        UnifiedModel {
            id: "model-1".to_owned(),
            display_name: "Model 1".to_owned(),
            kind,
            backend_id,
            capabilities: default_model_capabilities(
                kind,
                backend_id,
                "Model 1",
                &ModelSpec {
                    provider_id: provider_id.map(str::to_owned),
                    remote_model_id: remote_model_id.map(str::to_owned),
                    local_path: local_path.map(str::to_owned),
                    ..ModelSpec::default()
                },
            ),
            status,
            spec: ModelSpec {
                provider_id: provider_id.map(str::to_owned),
                remote_model_id: remote_model_id.map(str::to_owned),
                local_path: local_path.map(str::to_owned),
                ..ModelSpec::default()
            },
            runtime_presets: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
