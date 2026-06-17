mod catalog;
mod config_document;
mod download;
mod download_progress;
mod download_status;
mod pack;
mod runtime;

pub(crate) use catalog::list_chat_models_from_state;
pub(crate) use download::MODEL_DOWNLOAD_TASK_TYPE;
pub(crate) use runtime::{
    resolve_local_chat_prompt_profile, resolve_worker_model_backend_or_default,
};

use serde_json::{Map, Value};
use slab_types::RuntimeBackendId;

use crate::context::{ModelState, WorkerState};
use crate::domain::models::{
    CreateModelCommand, ModelConfigDocument, ModelConfigFieldScope, ModelConfigOrigin,
    ModelConfigSectionView, ModelConfigSourceSummary, ModelConfigValueType, UnifiedModel,
};
use crate::error::AppCoreError;
use crate::infra::model_packs;

use config_document::{
    build_model_config_field, build_model_config_inference_fields, build_model_config_section,
    build_model_config_selection_view, build_model_config_source_fields,
    build_model_config_source_summary, build_model_config_summary_fields, diffusion_load_origin,
    ensure_json_object, insert_optional_path, json_property_or_null, model_source_origin,
};

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

        let context = self.load_model_pack_context(id).await?;
        let selection = self.resolve_model_pack_selection(id, &context.resolved).await?;
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
                object.insert(
                    "chat_template".into(),
                    serde_json::to_value(chat_template).map_err(|error| {
                        AppCoreError::Internal(format!(
                            "failed to serialize load.chat_template for config document: {error}"
                        ))
                    })?,
                );
            }
            if let Some(gbnf) = bridge.load_defaults.gbnf.as_ref() {
                object.insert(
                    "gbnf".into(),
                    serde_json::to_value(gbnf).map_err(|error| {
                        AppCoreError::Internal(format!(
                            "failed to serialize load.gbnf for config document: {error}"
                        ))
                    })?,
                );
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
        let summary_fields = build_model_config_summary_fields(
            model,
            &command.display_name,
            bridge.backend.canonical_id().to_owned(),
            "Managed runtime backend selected for this pack.",
        )?;
        let source_fields = build_model_config_source_fields(source_summary, source_origin);

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
                    Some("Configured llama chat template asset reference.".into()),
                    ModelConfigValueType::Json,
                    json_property_or_null(resolved_load_spec, "chat_template"),
                    if bridge.load_defaults.chat_template.is_some() {
                        ModelConfigOrigin::SelectedBackendConfig
                    } else {
                        ModelConfigOrigin::Derived
                    },
                ));
                load_fields.push(build_model_config_field(
                    "load.gbnf",
                    ModelConfigFieldScope::Load,
                    "GBNF",
                    Some("Configured llama GBNF asset reference.".into()),
                    ModelConfigValueType::Json,
                    json_property_or_null(resolved_load_spec, "gbnf"),
                    if bridge.load_defaults.gbnf.is_some() {
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

        let inference_fields =
            build_model_config_inference_fields(resolved, resolved_inference_spec);

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
            build_model_config_section(
                "summary",
                "Summary",
                Some("Pack-backed catalog summary for the selected model.".into()),
                summary_fields,
            ),
            build_model_config_section(
                "source",
                "Source / Artifacts",
                Some("Resolved source and artifacts for the active selection.".into()),
                source_fields,
            ),
            build_model_config_section(
                "load",
                "Load",
                Some("Effective runtime load parameters.".into()),
                load_fields,
            ),
            build_model_config_section(
                "inference",
                "Inference",
                Some("Resolved inference defaults from the pack.".into()),
                inference_fields,
            ),
            build_model_config_section(
                "advanced",
                "Advanced",
                Some(
                    "Fallback JSON for fields not yet promoted into the canonical catalog.".into(),
                ),
                advanced_fields,
            ),
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
        let summary_fields = build_model_config_summary_fields(
            model,
            &command.display_name,
            backend_label,
            "Managed backend used for catalog projection and downloads.",
        )?;
        let source_fields = build_model_config_source_fields(source_summary, source_origin);

        let load_fields = vec![build_model_config_field(
            "load.runtime_load_supported",
            ModelConfigFieldScope::Load,
            "Runtime Load Supported",
            Some("Whether this pack resolves to a runtime-loadable model target.".into()),
            ModelConfigValueType::Boolean,
            Value::Bool(false),
            ModelConfigOrigin::Derived,
        )];

        let inference_fields =
            build_model_config_inference_fields(resolved, resolved_inference_spec);

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
            build_model_config_section(
                "summary",
                "Summary",
                Some("Pack-backed catalog summary for the selected model.".into()),
                summary_fields,
            ),
            build_model_config_section(
                "source",
                "Source / Artifacts",
                Some("Resolved source and artifacts for the active selection.".into()),
                source_fields,
            ),
            build_model_config_section(
                "load",
                "Load",
                Some(
                    "This pack does not expose a runtime-execution capability, so runtime load settings are unavailable."
                        .into(),
                ),
                load_fields,
            ),
            build_model_config_section(
                "inference",
                "Inference",
                Some("Resolved inference defaults from the pack.".into()),
                inference_fields,
            ),
            build_model_config_section(
                "advanced",
                "Advanced",
                Some("Additional metadata for the selected non-runtime pack.".into()),
                advanced_fields,
            ),
        ])
    }
}

#[cfg(test)]
mod tests;
