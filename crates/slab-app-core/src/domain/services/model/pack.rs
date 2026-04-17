use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use tracing::{info, warn};

use crate::domain::models::{
    CreateModelCommand, ManagedModelBackendId, ModelPackSelection, ModelSpec, RuntimePresets,
    SelectedModelDownloadSource, StoredModelConfig, UnifiedModel, UnifiedModelKind,
    UnifiedModelStatus,
};
use crate::error::AppCoreError;
use crate::infra::db::{ModelConfigStateRecord, ModelConfigStateStore, UnifiedModelRecord};
use crate::infra::model_packs;

use super::{ModelService, catalog};

#[derive(Debug, Clone)]
pub(super) struct ModelPackContext {
    pub(super) path: PathBuf,
    pub(super) resolved: slab_model_pack::ResolvedModelPack,
    pub(super) persisted: Option<StoredModelConfig>,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedModelPackSelectionView {
    pub(super) explicit_selection: ModelPackSelection,
    pub(super) effective_selection: ModelPackSelection,
    pub(super) selected_preset: slab_model_pack::ResolvedPreset,
    pub(super) warnings: Vec<String>,
    pub(super) legacy_selection_to_import: Option<ModelPackSelection>,
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

        let (command, legacy_selection) =
            match self.build_selected_model_pack_command(&summary.id, false).await {
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
            Ok(model) => {
                if let Some(record) = legacy_selection {
                    self.model_state.store().upsert_model_config_state(record).await?;
                }
                Ok(model)
            }
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
            let Some(model_id) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
            else {
                warn!(path = %path.display(), "skipping model pack without a valid file stem");
                continue;
            };

            let (command, legacy_selection) =
                match self.build_selected_model_pack_command(&model_id, false).await {
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
                    if let Some(record) = legacy_selection {
                        self.model_state.store().upsert_model_config_state(record).await?;
                    }
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

    pub(super) fn load_model_pack_context(
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
        let persisted = model_packs::read_persisted_model_config_from_pack(&pack_path)?;

        Ok(ModelPackContext { path: pack_path, resolved, persisted })
    }

    pub(super) async fn build_selected_model_pack_command(
        &self,
        id: &str,
        persist_legacy_selection: bool,
    ) -> Result<(CreateModelCommand, Option<ModelConfigStateRecord>), AppCoreError> {
        let context = self.load_model_pack_context(id)?;
        if matches!(
            context.resolved.manifest.sources.first().map(|candidate| &candidate.source),
            Some(slab_model_pack::PackSource::Cloud { .. })
        ) {
            let command = model_packs::build_model_command_from_pack(&context.path)?;
            return Ok((command, None));
        }

        let selection = self
            .resolve_model_pack_selection(
                id,
                &context.resolved,
                context.persisted.as_ref(),
                persist_legacy_selection,
            )
            .await?;
        let command = build_model_command_from_pack_context(&context, &selection.selected_preset)?;
        let state_record = selection.legacy_selection_to_import.map(|selection| {
            model_config_state_record(id, selection.preset_id, selection.variant_id)
        });

        Ok((command, state_record))
    }

    pub(super) async fn resolve_model_pack_selection(
        &self,
        model_id: &str,
        resolved: &slab_model_pack::ResolvedModelPack,
        persisted: Option<&StoredModelConfig>,
        persist_legacy_selection: bool,
    ) -> Result<ResolvedModelPackSelectionView, AppCoreError> {
        let state_record = self.model_state.store().get_model_config_state(model_id).await?;
        let legacy_selection = persisted
            .and_then(|config| config.pack_selection.clone())
            .map(normalize_model_pack_selection);

        let explicit_selection = if let Some(record) = state_record.as_ref() {
            ModelPackSelection {
                preset_id: catalog::normalize_optional_text(record.selected_preset_id.clone()),
                variant_id: catalog::normalize_optional_text(record.selected_variant_id.clone()),
            }
        } else {
            legacy_selection.clone().unwrap_or_default()
        };

        let (effective_selection, selected_preset, warnings) =
            resolve_effective_model_pack_selection(resolved, &explicit_selection)?;

        let legacy_selection_to_import = if state_record.is_none() {
            legacy_selection
                .as_ref()
                .filter(|selection| {
                    effective_model_pack_selection(resolved, selection, &selected_preset)
                        != default_model_pack_selection(resolved)
                })
                .cloned()
        } else {
            None
        };

        if persist_legacy_selection
            && state_record.is_none()
            && let Some(selection) = legacy_selection_to_import.as_ref()
        {
            self.model_state
                .store()
                .upsert_model_config_state(model_config_state_record(
                    model_id,
                    selection.preset_id.clone(),
                    selection.variant_id.clone(),
                ))
                .await?;
        }

        Ok(ResolvedModelPackSelectionView {
            explicit_selection,
            effective_selection,
            selected_preset,
            warnings,
            legacy_selection_to_import: if persist_legacy_selection {
                None
            } else {
                legacy_selection_to_import
            },
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
        .backend_hints
        .prefer_drivers
        .iter()
        .find_map(|driver| driver.parse::<ManagedModelBackendId>().ok())
        .or_else(|| default_managed_backend_for_pack_family(manifest.family))
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "pack '{}' declares only non-runtime capabilities; add a managed backend hint such as ggml.whisper",
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

fn runtime_presets_from_manifest(
    manifest: &slab_model_pack::ModelPackManifest,
) -> Option<RuntimePresets> {
    manifest.runtime_presets.as_ref().and_then(|presets| {
        (presets.max_tokens.is_some()
            || presets.temperature.is_some()
            || presets.top_p.is_some()
            || presets.top_k.is_some()
            || presets.min_p.is_some()
            || presets.presence_penalty.is_some()
            || presets.repetition_penalty.is_some())
        .then_some(RuntimePresets {
            max_tokens: presets.max_tokens,
            temperature: presets.temperature,
            top_p: presets.top_p,
            top_k: presets.top_k,
            min_p: presets.min_p,
            presence_penalty: presets.presence_penalty,
            repetition_penalty: presets.repetition_penalty,
        })
    })
}

pub(super) fn primary_materialized_artifact_path(config: &StoredModelConfig) -> Option<String> {
    catalog::primary_artifact_key(&config.materialized_artifacts)
        .and_then(|key| config.materialized_artifacts.get(&key).cloned())
}

pub(super) fn sync_model_pack_record(
    config_dir: &Path,
    record: UnifiedModelRecord,
    materialized_artifacts: Option<BTreeMap<String, String>>,
    selected_download_source: Option<SelectedModelDownloadSource>,
) -> Result<(), AppCoreError> {
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    let mut config: StoredModelConfig = model.into();
    let existing_path = model_packs::model_pack_file_path(config_dir, &config.id);
    let existing = if existing_path.exists() {
        model_packs::read_persisted_model_config_from_pack(&existing_path)?
    } else {
        None
    };

    if let Some(materialized_artifacts) = materialized_artifacts {
        config.materialized_artifacts = materialized_artifacts;
        if config.spec.local_path.is_none() {
            config.spec.local_path = primary_materialized_artifact_path(&config);
        }
    } else if let Some(existing) = existing.as_ref() {
        config.materialized_artifacts = existing.materialized_artifacts.clone();
    }

    if let Some(selected_download_source) = selected_download_source {
        apply_selected_download_source_to_spec(&mut config.spec, &selected_download_source);
        config.selected_download_source = Some(selected_download_source);
    } else if let Some(existing) = existing {
        config.selected_download_source = existing.selected_download_source;
    }

    model_packs::write_persisted_model_pack_from_config(config_dir, &config)?;
    Ok(())
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
    document.variant_id = Some(variant_id.to_owned());

    let effective_load_config = if base_preset.document.load_config.is_some() {
        base_preset.effective_load_config.clone()
    } else {
        selected_variant.load_config.clone()
    };
    let effective_inference_config = if base_preset.document.inference_config.is_some() {
        base_preset.effective_inference_config.clone()
    } else {
        selected_variant.inference_config.clone()
    };

    Ok(slab_model_pack::ResolvedPreset {
        document,
        variant: selected_variant,
        adapters: base_preset.adapters.clone(),
        effective_load_config,
        effective_inference_config,
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
            let status = manifest
                .status
                .map(|status| match status {
                    slab_model_pack::PackModelStatus::Ready => UnifiedModelStatus::Ready,
                    slab_model_pack::PackModelStatus::NotDownloaded => {
                        UnifiedModelStatus::NotDownloaded
                    }
                    slab_model_pack::PackModelStatus::Downloading => {
                        UnifiedModelStatus::Downloading
                    }
                    slab_model_pack::PackModelStatus::Error => UnifiedModelStatus::Error,
                })
                .unwrap_or_else(|| match &bridge.model_spec.source {
                    slab_types::ModelSource::HuggingFace { .. } => {
                        UnifiedModelStatus::NotDownloaded
                    }
                    _ => UnifiedModelStatus::Ready,
                });
            let runtime_presets = runtime_presets_from_manifest(manifest).or_else(|| {
                let max_tokens = bridge
                    .inference_defaults
                    .get("max_tokens")
                    .and_then(|value| value.as_u64().and_then(|value| u32::try_from(value).ok()));
                let temperature = bridge
                    .inference_defaults
                    .get("temperature")
                    .and_then(|value| value.as_f64().map(|value| value as f32));
                let top_p = bridge
                    .inference_defaults
                    .get("top_p")
                    .and_then(|value| value.as_f64().map(|value| value as f32));
                let top_k = bridge
                    .inference_defaults
                    .get("top_k")
                    .and_then(|value| value.as_i64().and_then(|value| i32::try_from(value).ok()));
                let min_p = bridge
                    .inference_defaults
                    .get("min_p")
                    .and_then(|value| value.as_f64().map(|value| value as f32));
                let presence_penalty = bridge
                    .inference_defaults
                    .get("presence_penalty")
                    .and_then(|value| value.as_f64().map(|value| value as f32));
                let repetition_penalty = bridge
                    .inference_defaults
                    .get("repetition_penalty")
                    .and_then(|value| value.as_f64().map(|value| value as f32));
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
            });
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
            let status = manifest
                .status
                .map(|status| match status {
                    slab_model_pack::PackModelStatus::Ready => UnifiedModelStatus::Ready,
                    slab_model_pack::PackModelStatus::NotDownloaded => {
                        UnifiedModelStatus::NotDownloaded
                    }
                    slab_model_pack::PackModelStatus::Downloading => {
                        UnifiedModelStatus::Downloading
                    }
                    slab_model_pack::PackModelStatus::Error => UnifiedModelStatus::Error,
                })
                .unwrap_or_else(|| match &source {
                    slab_types::ModelSource::HuggingFace { .. } => {
                        UnifiedModelStatus::NotDownloaded
                    }
                    _ => UnifiedModelStatus::Ready,
                });
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
                runtime_presets: runtime_presets_from_manifest(manifest),
            })
        }
        Err(error) => Err(AppCoreError::BadRequest(format!(
            "failed to compile selected pack preset: {error}"
        ))),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ModelSourcePreview {
    pub(super) repo_id: Option<String>,
    pub(super) filename: Option<String>,
    pub(super) hub_provider: Option<String>,
    pub(super) local_path: Option<String>,
}

impl ModelSourcePreview {
    fn into_model_spec(self) -> ModelSpec {
        ModelSpec {
            repo_id: self.repo_id,
            filename: self.filename,
            hub_provider: self.hub_provider,
            local_path: self.local_path,
            ..Default::default()
        }
    }

    fn is_empty(&self) -> bool {
        self.repo_id.is_none() && self.filename.is_none() && self.local_path.is_none()
    }
}

fn canonical_hub_provider(value: Option<&str>) -> Option<String> {
    catalog::normalized_hub_provider_preference(value).ok().and_then(|(_, canonical)| canonical)
}

fn comparable_hub_provider(spec: &ModelSpec) -> Option<String> {
    let has_remote_source =
        spec.repo_id.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some()
            && spec.filename.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some();

    canonical_hub_provider(spec.hub_provider.as_deref())
        .or_else(|| has_remote_source.then(|| "hf_hub".to_owned()))
}

fn pack_source_hub_provider(source: &slab_model_pack::PackSource) -> Option<String> {
    match source {
        slab_model_pack::PackSource::HuggingFace { .. } => Some("hf_hub".to_owned()),
        slab_model_pack::PackSource::ModelScope { .. } => Some("models_cat".to_owned()),
        slab_model_pack::PackSource::LocalPath { .. }
        | slab_model_pack::PackSource::LocalFiles { .. }
        | slab_model_pack::PackSource::Cloud { .. } => None,
    }
}

pub(super) fn source_preview_from_pack_source(
    source: Option<&slab_model_pack::PackSourceCandidate>,
) -> ModelSourcePreview {
    match source.map(|candidate| &candidate.source) {
        Some(
            source @ (slab_model_pack::PackSource::HuggingFace { .. }
            | slab_model_pack::PackSource::ModelScope { .. }),
        ) => {
            let remote_source = source
                .remote_repository()
                .expect("remote source candidates expose repository info");
            ModelSourcePreview {
                repo_id: Some(remote_source.repo_id.to_owned()),
                filename: remote_source
                    .files
                    .iter()
                    .find(|file| file.id == "model")
                    .or_else(|| remote_source.files.first())
                    .map(|file| file.path.clone()),
                hub_provider: pack_source_hub_provider(source),
                local_path: None,
            }
        }
        Some(slab_model_pack::PackSource::LocalPath { path }) => {
            ModelSourcePreview { local_path: Some(path.clone()), ..Default::default() }
        }
        Some(slab_model_pack::PackSource::LocalFiles { files }) => ModelSourcePreview {
            local_path: files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone()),
            ..Default::default()
        },
        Some(slab_model_pack::PackSource::Cloud { .. }) | None => ModelSourcePreview::default(),
    }
}

fn source_preview_from_model_source(
    source: &slab_types::ModelSource,
    hub_provider: Option<&str>,
) -> ModelSourcePreview {
    match source {
        slab_types::ModelSource::HuggingFace { repo_id, files, .. } => ModelSourcePreview {
            repo_id: Some(repo_id.clone()),
            filename: files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            hub_provider: canonical_hub_provider(hub_provider),
            local_path: None,
        },
        slab_types::ModelSource::LocalPath { path } => ModelSourcePreview {
            local_path: Some(path.to_string_lossy().into_owned()),
            ..Default::default()
        },
        slab_types::ModelSource::LocalArtifacts { files } => ModelSourcePreview {
            local_path: files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            ..Default::default()
        },
        _ => ModelSourcePreview::default(),
    }
}

fn preview_from_pack_candidate_or_model_source(
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
    source: &slab_types::ModelSource,
) -> ModelSourcePreview {
    let preview = source_preview_from_pack_source(source_hint);
    if preview.is_empty() { source_preview_from_model_source(source, None) } else { preview }
}

pub(super) fn materialized_model_source(
    source: &slab_types::ModelSource,
    persisted: Option<&StoredModelConfig>,
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
) -> slab_types::ModelSource {
    let Some(persisted) = persisted else {
        return source.clone();
    };
    let projected_spec =
        preview_from_pack_candidate_or_model_source(source_hint, source).into_model_spec();
    if !same_model_download_source(&persisted.spec, &projected_spec) {
        return source.clone();
    }

    if !persisted.materialized_artifacts.is_empty() {
        return slab_types::ModelSource::LocalArtifacts {
            files: persisted
                .materialized_artifacts
                .iter()
                .map(|(artifact_id, path)| (artifact_id.clone(), PathBuf::from(path)))
                .collect(),
        };
    }

    let Some(local_path) =
        persisted.spec.local_path.as_deref().map(str::trim).filter(|value| !value.is_empty())
    else {
        return source.clone();
    };

    match source {
        slab_types::ModelSource::HuggingFace { .. }
        | slab_types::ModelSource::LocalPath { .. }
        | slab_types::ModelSource::LocalArtifacts { .. } => {
            slab_types::ModelSource::LocalPath { path: PathBuf::from(local_path) }
        }
        _ => source.clone(),
    }
}

pub(super) fn apply_materialized_source_to_bridge(
    bridge: &mut slab_model_pack::ModelPackRuntimeBridge,
    persisted: Option<&StoredModelConfig>,
    source_hint: Option<&slab_model_pack::PackSourceCandidate>,
) {
    bridge.model_spec.source =
        materialized_model_source(&bridge.model_spec.source, persisted, source_hint);
}

pub(super) fn apply_selected_download_source_to_spec(
    spec: &mut ModelSpec,
    selected_download_source: &SelectedModelDownloadSource,
) {
    spec.repo_id = Some(selected_download_source.repo_id.clone());
    spec.filename = Some(selected_download_source.filename.clone());
    spec.hub_provider = selected_download_source.hub_provider.clone();
}

pub(super) fn same_model_download_source(current: &ModelSpec, next: &ModelSpec) -> bool {
    match (current.repo_id.as_deref(), next.repo_id.as_deref()) {
        (Some(_), Some(_)) => {
            current.repo_id == next.repo_id
                && current.filename == next.filename
                && comparable_hub_provider(current) == comparable_hub_provider(next)
        }
        (None, None) => current.local_path == next.local_path,
        _ => false,
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
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{materialized_model_source, same_model_download_source};
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        ModelSpec, StoredModelConfig, UnifiedModelKind, UnifiedModelStatus,
    };
    use slab_model_pack::{PackSource, PackSourceCandidate, PackSourceFile};

    #[test]
    fn same_model_download_source_treats_legacy_blank_provider_as_hf_hub() {
        let persisted = ModelSpec {
            repo_id: Some("ggml-org/whisper-vad".into()),
            filename: Some("ggml-silero-v6.2.0.bin".into()),
            hub_provider: Some("hf_hub".into()),
            ..ModelSpec::default()
        };
        let projected = ModelSpec {
            repo_id: Some("ggml-org/whisper-vad".into()),
            filename: Some("ggml-silero-v6.2.0.bin".into()),
            hub_provider: None,
            ..ModelSpec::default()
        };

        assert!(same_model_download_source(&persisted, &projected));
    }

    #[test]
    fn materialized_model_source_uses_pack_source_provider_hint_for_modelscope() {
        let mut files = BTreeMap::new();
        files.insert("model".to_owned(), PathBuf::from("Qwen2.5-7B-Instruct-Q4_K_M.gguf"));

        let source = slab_types::ModelSource::HuggingFace {
            repo_id: "Qwen/Qwen2.5-7B-Instruct".into(),
            revision: None,
            files,
        };
        let persisted = StoredModelConfig {
            schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
            policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION,
            id: "local-qwen".into(),
            display_name: "Local Qwen".into(),
            kind: UnifiedModelKind::Local,
            backend_id: None,
            capabilities: Vec::new(),
            status: Some(UnifiedModelStatus::Ready),
            spec: ModelSpec {
                repo_id: Some("Qwen/Qwen2.5-7B-Instruct".into()),
                filename: Some("Qwen2.5-7B-Instruct-Q4_K_M.gguf".into()),
                hub_provider: Some("models_cat".into()),
                local_path: Some("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf".into()),
                ..ModelSpec::default()
            },
            runtime_presets: None,
            materialized_artifacts: BTreeMap::new(),
            selected_download_source: None,
            pack_selection: None,
        };
        let source_hint = PackSourceCandidate::new(PackSource::ModelScope {
            repo_id: "Qwen/Qwen2.5-7B-Instruct".into(),
            revision: None,
            files: vec![PackSourceFile {
                id: "model".into(),
                label: None,
                description: None,
                path: "Qwen2.5-7B-Instruct-Q4_K_M.gguf".into(),
            }],
        });

        let materialized = materialized_model_source(&source, Some(&persisted), Some(&source_hint));

        assert_eq!(
            materialized,
            slab_types::ModelSource::LocalPath {
                path: PathBuf::from("C:/models/Qwen2.5-7B-Instruct-Q4_K_M.gguf")
            }
        );
    }
}
