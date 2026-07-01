mod source;

use source::preview_from_pack_candidate_or_model_source;
pub(super) use source::{
    apply_materialized_source_to_bridge, apply_selected_download_source_to_spec,
    materialized_model_source, same_model_download_source, source_preview_from_pack_source,
};

use std::path::Path;

use chrono::Utc;
use tracing::{info, warn};

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelPackSelection, ModelSpec, RuntimePresets,
    StoredModelConfig, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
};
use crate::error::AppCoreError;
use crate::infra::db::{ModelConfigStateRecord, ModelConfigStateStore, ModelStore};
use crate::infra::model_packs;

use super::{ModelService, catalog};

#[derive(Debug, Clone)]
pub(super) struct ModelPackContext {
    pub(super) resolved: slab_model_pack::ResolvedModelPack,
    pub(super) persisted: Option<StoredModelConfig>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedModelPackSelectionView {
    pub(super) explicit_selection: ModelPackSelection,
    pub(super) effective_selection: ModelPackSelection,
    pub(super) selected_preset: slab_model_pack::ResolvedPreset,
    pub(super) warnings: Vec<String>,
}

impl ModelService {
    pub async fn import_model_pack_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<UnifiedModel, AppCoreError> {
        let summary = model_packs::read_model_pack_summary_from_bytes(bytes)?;
        let pack_path = model_packs::model_pack_file_path(self.model_config_dir(), &summary.id);
        let pack_existed = pack_path.exists();
        model_packs::write_model_pack(self.model_config_dir(), &summary.id, bytes)?;

        let command = match self.build_selected_model_pack_command(&summary.id).await {
            Ok(result) => result,
            Err(error) => {
                if !pack_existed {
                    let _ = model_packs::delete_model_pack_at_path(&pack_path);
                }
                return Err(error);
            }
        };

        let model = self.build_model_definition(command).await?;

        match self.store_model_definition(model).await {
            Ok(model) => Ok(model),
            Err(error) => {
                if !pack_existed {
                    let _ = model_packs::delete_model_pack_at_path(&pack_path);
                }
                Err(error)
            }
        }
    }

    pub async fn sync_model_packs_from_disk(&self) -> Result<(), AppCoreError> {
        let config_dir = self.model_config_dir().to_path_buf();
        let pack_paths = model_packs::list_model_pack_paths(&config_dir)?;
        if pack_paths.is_empty() {
            info!(path = %config_dir.display(), "no model pack files found during startup");
            return Ok(());
        }

        let mut imported = 0usize;

        for path in pack_paths {
            let model_id = match model_packs::read_model_pack_summary(&path) {
                Ok(summary) => summary.id,
                Err(error) => {
                    warn!(
                        path = %path.display(),
                        error = %error,
                        "skipping invalid model pack file"
                    );
                    continue;
                }
            };

            let command = match self.build_selected_model_pack_command(&model_id).await {
                Ok(command) => command,
                Err(error) => {
                    warn!(
                        path = %path.display(),
                        model_id = %model_id,
                        error = %error,
                        "skipping invalid model pack file"
                    );
                    continue;
                }
            };

            match self.persist_model_definition_with_options(command, false).await {
                Ok(model) => {
                    imported += 1;
                    info!(model_id = %model.id, path = %path.display(), "initialized model from .slab pack");
                }
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "failed to initialize model from .slab pack");
                }
            }
        }

        info!(
            path = %config_dir.display(),
            imported,
            "model pack startup sync complete"
        );
        Ok(())
    }

    pub(super) fn model_config_dir(&self) -> &Path {
        self.model_state.config().model_config_dir.as_path()
    }

    pub(super) async fn load_model_pack_context(
        &self,
        id: &str,
    ) -> Result<ModelPackContext, AppCoreError> {
        let pack_path = model_packs::model_pack_file_path(self.model_config_dir(), id);
        if !pack_path.exists() {
            return Err(AppCoreError::NotFound(format!(
                "model pack for '{id}' was not found on disk"
            )));
        }

        let pack = model_packs::open_model_pack(&pack_path)?;
        let resolved = pack.resolve().map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to resolve model pack '{}': {error}",
                pack_path.display()
            ))
        })?;
        let persisted = read_model_download_state_from_db(&self.model_state, id)
            .await?
            .or(model_packs::read_persisted_model_config_from_pack(&pack_path)?);

        Ok(ModelPackContext { resolved, persisted })
    }

    pub(super) async fn build_selected_model_pack_command(
        &self,
        id: &str,
    ) -> Result<CreateModelCommand, AppCoreError> {
        let context = self.load_model_pack_context(id).await?;
        let selection = self.resolve_model_pack_selection(id, &context.resolved).await?;
        let command = build_model_command_from_pack_context(&context, &selection.selected_preset)?;

        Ok(command)
    }

    pub(super) async fn resolve_model_pack_selection(
        &self,
        model_id: &str,
        resolved: &slab_model_pack::ResolvedModelPack,
    ) -> Result<ResolvedModelPackSelectionView, AppCoreError> {
        let state_record = self.model_state.store().get_model_config_state(model_id).await?;
        let explicit_selection = if let Some(record) = state_record.as_ref() {
            ModelPackSelection {
                preset_id: catalog::normalize_optional_text(record.selected_preset_id.clone()),
                variant_id: catalog::normalize_optional_text(record.selected_variant_id.clone()),
            }
        } else {
            ModelPackSelection::default()
        };

        let (effective_selection, selected_preset, warnings) =
            resolve_effective_model_pack_selection(resolved, &explicit_selection)?;

        Ok(ResolvedModelPackSelectionView {
            explicit_selection,
            effective_selection,
            selected_preset,
            warnings,
        })
    }
}

pub(super) fn pack_has_runtime_execution_capability(
    manifest: &slab_model_pack::ModelPackManifest,
) -> bool {
    manifest.capabilities.iter().any(|capability| capability.is_runtime_execution())
}

fn default_managed_backend_for_pack_family(
    family: slab_types::ModelFamily,
) -> Option<ManagedModelBackendId> {
    match family {
        slab_types::ModelFamily::Llama => Some(ManagedModelBackendId::GgmlLlama),
        slab_types::ModelFamily::Whisper => Some(ManagedModelBackendId::GgmlWhisper),
        slab_types::ModelFamily::Diffusion => Some(ManagedModelBackendId::GgmlDiffusion),
        slab_types::ModelFamily::Onnx => None,
        _ => None,
    }
}

pub(super) fn resolve_projection_backend_for_pack(
    manifest: &slab_model_pack::ModelPackManifest,
) -> Result<ManagedModelBackendId, AppCoreError> {
    manifest
        .engines
        .iter()
        .find_map(|engine| ManagedModelBackendId::try_from(engine.id).ok())
        .or_else(|| default_managed_backend_for_pack_family(manifest.family))
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "pack '{}' declares only non-runtime capabilities; add a managed engine such as ggml.whisper",
                manifest.id
            ))
        })
}

pub(super) fn resolve_pack_model_source(
    resolved: &slab_model_pack::ResolvedModelPack,
    preset: &slab_model_pack::ResolvedPreset,
    error_context: &str,
) -> Result<slab_types::ModelSource, AppCoreError> {
    resolved
        .compile_model_source(preset)
        .map_err(|error| AppCoreError::BadRequest(format!("{error_context}: {error}")))
}

fn runtime_presets_from_inference_defaults(
    bridge: &slab_model_pack::ModelPackRuntimeBridge,
) -> Option<RuntimePresets> {
    RuntimePresets::from_json_options(&bridge.inference_defaults)
}

pub(super) fn primary_materialized_artifact_path(config: &StoredModelConfig) -> Option<String> {
    catalog::primary_artifact_key(&config.materialized_artifacts)
        .and_then(|key| config.materialized_artifacts.get(&key).cloned())
}

pub(super) async fn read_model_download_state_from_db(
    state: &crate::context::ModelState,
    model_id: &str,
) -> Result<Option<StoredModelConfig>, AppCoreError> {
    let Some(record) = state.store().get_model(model_id).await? else {
        return Ok(None);
    };
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    let config: StoredModelConfig = model.into();

    Ok(has_model_download_state(&config).then_some(config))
}

fn has_model_download_state(config: &StoredModelConfig) -> bool {
    config.spec.local_path.as_deref().is_some_and(|path| !path.trim().is_empty())
        || !config.materialized_artifacts.is_empty()
        || config.selected_download_source.is_some()
}

pub(super) fn default_model_pack_selection(
    resolved: &slab_model_pack::ResolvedModelPack,
) -> ModelPackSelection {
    let default_preset = resolved.default_preset();

    ModelPackSelection {
        preset_id: resolved.default_preset_id.clone(),
        variant_id: default_preset
            .and_then(|preset| non_empty_variant_id(&preset.variant.document.id)),
    }
}

pub(super) fn non_empty_variant_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(super) fn normalize_model_pack_selection(selection: ModelPackSelection) -> ModelPackSelection {
    ModelPackSelection {
        preset_id: catalog::normalize_optional_text(selection.preset_id),
        variant_id: catalog::normalize_optional_text(selection.variant_id),
    }
}

pub(super) fn resolve_selected_model_pack_preset(
    resolved: &slab_model_pack::ResolvedModelPack,
    selection: &ModelPackSelection,
) -> Result<slab_model_pack::ResolvedPreset, AppCoreError> {
    let base_preset = if let Some(preset_id) = selection.preset_id.as_deref() {
        resolved.presets.get(preset_id).cloned().ok_or_else(|| {
            AppCoreError::BadRequest(format!("model pack preset '{preset_id}' was not found"))
        })?
    } else {
        resolved.default_preset().cloned().ok_or_else(|| {
            AppCoreError::BadRequest(
                "model pack has no configurable preset; enhancement is unavailable".into(),
            )
        })?
    };

    let Some(variant_id) = selection.variant_id.as_deref() else {
        return Ok(base_preset);
    };

    let selected_variant = resolved.variants.get(variant_id).cloned().ok_or_else(|| {
        AppCoreError::BadRequest(format!("model pack variant '{variant_id}' was not found"))
    })?;

    let mut document = base_preset.document.clone();
    document.variant_id = variant_id.to_owned();
    let engine_candidates = resolved
        .manifest
        .engines
        .iter()
        .copied()
        .filter(|engine| engine.format == selected_variant.document.format)
        .collect::<Vec<_>>();
    if engine_candidates.is_empty() {
        return Err(AppCoreError::BadRequest(format!(
            "model pack variant '{variant_id}' has no compatible engine"
        )));
    }

    Ok(slab_model_pack::ResolvedPreset {
        document,
        variant: selected_variant,
        adapters: base_preset.adapters.clone(),
        effective_inference_config: base_preset.effective_inference_config.clone(),
        engine_candidates,
    })
}

pub(super) fn build_local_model_command_from_pack_preset(
    manifest: &slab_model_pack::ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
    preset: &slab_model_pack::ResolvedPreset,
) -> Result<CreateModelCommand, AppCoreError> {
    match resolved.compile_runtime_bridge(preset) {
        Ok(bridge) => {
            let backend_id = ManagedModelBackendId::try_from(bridge.backend).map_err(|error| {
                AppCoreError::BadRequest(format!(
                    "model pack backend '{}' is not supported by managed local models: {}",
                    bridge.backend, error
                ))
            })?;
            let status = match &bridge.model_spec.source {
                slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
                _ => UnifiedModelStatus::Ready,
            };
            let runtime_presets = runtime_presets_from_inference_defaults(&bridge);
            let source_preview = preview_from_pack_candidate_or_model_source(
                preset.variant.effective_sources.first(),
                &bridge.model_spec.source,
            );
            let allow_local_path_fallback = source_preview.repo_id.is_none();

            Ok(CreateModelCommand {
                id: Some(manifest.id.clone()),
                display_name: manifest.label.clone(),
                kind: UnifiedModelKind::Local,
                backend_id: Some(backend_id),
                capabilities: Some(manifest.capabilities.clone()),
                status: Some(status),
                spec: ModelSpec {
                    pricing: manifest.pricing.as_ref().map(|pricing| {
                        crate::domain::models::Pricing {
                            input: pricing.input,
                            output: pricing.output,
                        }
                    }),
                    repo_id: source_preview.repo_id,
                    filename: source_preview.filename,
                    hub_provider: source_preview.hub_provider,
                    local_path: source_preview.local_path.or_else(|| {
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
        Err(slab_model_pack::ModelPackError::MissingRuntimeCapability)
            if !pack_has_runtime_execution_capability(manifest) =>
        {
            let source = resolve_pack_model_source(
                resolved,
                preset,
                "failed to resolve selected pack preset source",
            )?;
            let backend_id = resolve_projection_backend_for_pack(manifest)?;
            let status = match &source {
                slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
                _ => UnifiedModelStatus::Ready,
            };
            let source_preview = preview_from_pack_candidate_or_model_source(
                preset.variant.effective_sources.first(),
                &source,
            );
            let allow_local_path_fallback = source_preview.repo_id.is_none();

            Ok(CreateModelCommand {
                id: Some(manifest.id.clone()),
                display_name: manifest.label.clone(),
                kind: UnifiedModelKind::Local,
                backend_id: Some(backend_id),
                capabilities: Some(manifest.capabilities.clone()),
                status: Some(status),
                spec: ModelSpec {
                    pricing: manifest.pricing.as_ref().map(|pricing| {
                        crate::domain::models::Pricing {
                            input: pricing.input,
                            output: pricing.output,
                        }
                    }),
                    repo_id: source_preview.repo_id,
                    filename: source_preview.filename,
                    hub_provider: source_preview.hub_provider,
                    local_path: source_preview.local_path.or_else(|| {
                        allow_local_path_fallback
                            .then(|| {
                                source
                                    .primary_path()
                                    .map(|value| value.to_string_lossy().into_owned())
                            })
                            .flatten()
                    }),
                    context_window: manifest.context_window,
                    ..Default::default()
                },
                runtime_presets: None,
            })
        }
        Err(error) => Err(AppCoreError::BadRequest(format!(
            "failed to compile selected pack preset: {error}"
        ))),
    }
}

pub(super) fn build_model_command_from_pack_context(
    context: &ModelPackContext,
    preset: &slab_model_pack::ResolvedPreset,
) -> Result<CreateModelCommand, AppCoreError> {
    let mut command = build_local_model_command_from_pack_preset(
        &context.resolved.manifest,
        &context.resolved,
        preset,
    )?;
    if let Some(persisted) = context.persisted.as_ref() {
        apply_persisted_projection_state(&mut command, persisted);
    }
    Ok(command)
}

fn apply_persisted_projection_state(
    command: &mut CreateModelCommand,
    persisted: &StoredModelConfig,
) {
    if let Some(selected_download_source) = persisted.selected_download_source.as_ref() {
        apply_selected_download_source_to_spec(&mut command.spec, selected_download_source);
        command.spec.local_path = persisted
            .spec
            .local_path
            .clone()
            .or_else(|| primary_materialized_artifact_path(persisted));
        if let Some(status) = persisted.status.clone() {
            command.status = Some(status);
        }
        return;
    }

    if same_model_download_source(&persisted.spec, &command.spec) {
        command.spec.local_path = persisted
            .spec
            .local_path
            .clone()
            .or_else(|| primary_materialized_artifact_path(persisted));
        if let Some(status) = persisted.status.clone() {
            command.status = Some(status);
        }
    }
}

pub(super) fn resolve_effective_model_pack_selection(
    resolved: &slab_model_pack::ResolvedModelPack,
    explicit_selection: &ModelPackSelection,
) -> Result<(ModelPackSelection, slab_model_pack::ResolvedPreset, Vec<String>), AppCoreError> {
    let default_selection = default_model_pack_selection(resolved);
    let mut warnings = Vec::new();

    let preset_id = match explicit_selection.preset_id.as_deref() {
        Some(preset_id) if resolved.presets.contains_key(preset_id) => Some(preset_id.to_owned()),
        Some(preset_id) => {
            warnings.push(format!(
                "Preset '{preset_id}' is no longer available. Selection was reset to pack default."
            ));
            default_selection.preset_id.clone()
        }
        None => default_selection.preset_id.clone(),
    };

    let base_selection = ModelPackSelection { preset_id: preset_id.clone(), variant_id: None };
    let base_preset = resolve_selected_model_pack_preset(resolved, &base_selection)?;
    let default_variant_id = non_empty_variant_id(&base_preset.variant.document.id);

    let variant_id = match explicit_selection.variant_id.as_deref() {
        Some(variant_id) if resolved.variants.contains_key(variant_id) => {
            Some(variant_id.to_owned())
        }
        Some(variant_id) => {
            warnings.push(format!(
                "Variant '{variant_id}' is no longer available. Selection was reset to pack default."
            ));
            default_variant_id.clone()
        }
        None => default_variant_id.clone(),
    };

    let effective_selection = ModelPackSelection { preset_id, variant_id };
    let selected_preset = resolve_selected_model_pack_preset(resolved, &effective_selection)?;

    Ok((effective_selection, selected_preset, warnings))
}

pub(super) fn effective_model_pack_selection(
    resolved: &slab_model_pack::ResolvedModelPack,
    explicit_selection: &ModelPackSelection,
    selected_preset: &slab_model_pack::ResolvedPreset,
) -> ModelPackSelection {
    ModelPackSelection {
        preset_id: explicit_selection
            .preset_id
            .clone()
            .or_else(|| resolved.default_preset_id.clone()),
        variant_id: explicit_selection
            .variant_id
            .clone()
            .or_else(|| non_empty_variant_id(&selected_preset.variant.document.id)),
    }
}

pub(super) fn selection_state_record_for_storage(
    model_id: &str,
    resolved: &slab_model_pack::ResolvedModelPack,
    explicit_selection: &ModelPackSelection,
    effective_selection: &ModelPackSelection,
) -> Option<ModelConfigStateRecord> {
    (effective_selection != &default_model_pack_selection(resolved)).then(|| {
        model_config_state_record(
            model_id,
            explicit_selection.preset_id.clone(),
            explicit_selection.variant_id.clone(),
        )
    })
}

pub(super) fn model_config_state_record(
    model_id: &str,
    selected_preset_id: Option<String>,
    selected_variant_id: Option<String>,
) -> ModelConfigStateRecord {
    ModelConfigStateRecord {
        model_id: model_id.to_owned(),
        selected_preset_id,
        selected_variant_id,
        selected_engine_id: None,
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::models::{UnifiedModelKind, UnifiedModelStatus};
    use crate::error::AppCoreError;
    use crate::test_support::{TestAppCore, local_model_pack_bytes};

    #[tokio::test]
    async fn model_pack_import_model_pack_bytes_persists_local_model_and_pack() {
        let app = TestAppCore::new().await;
        let bytes = local_model_pack_bytes("pack-import-local");

        let model = app.model.import_model_pack_bytes(&bytes).await.expect("import pack");

        assert_eq!(model.id, "pack-import-local");
        assert_eq!(model.kind, UnifiedModelKind::Local);
        assert_eq!(model.status, UnifiedModelStatus::NotDownloaded);
        assert_eq!(model.spec.repo_id.as_deref(), Some("bartowski/Qwen2.5-7B-Instruct-GGUF"));
        assert!(app.model_pack_path(&model.id).is_file());

        let fetched = app.model.get_model(&model.id).await.expect("fetch imported model");
        assert_eq!(fetched.id, model.id);
        assert_eq!(fetched.kind, UnifiedModelKind::Local);
    }

    #[tokio::test]
    async fn model_pack_sync_model_packs_from_disk_imports_valid_and_skips_invalid() {
        let app = TestAppCore::new().await;
        std::fs::write(
            app.model_pack_path("pack-sync-local"),
            local_model_pack_bytes("pack-sync-local"),
        )
        .expect("write valid pack");
        std::fs::write(app.model_config_dir.join("invalid.slab"), b"not a slab pack")
            .expect("write invalid pack");

        app.model.sync_model_packs_from_disk().await.expect("sync packs");

        let synced = app.model.get_model("pack-sync-local").await.expect("synced model");
        assert_eq!(synced.kind, UnifiedModelKind::Local);
        let invalid = app.model.get_model("invalid").await.expect_err("invalid pack skipped");
        assert!(
            matches!(&invalid, AppCoreError::NotFound(message) if message.contains("model invalid not found")),
            "unexpected error: {invalid}"
        );
    }
}
