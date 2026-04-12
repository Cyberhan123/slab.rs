use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use hf_hub::api::sync::{Api, ApiBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use slab_proto::convert;
use slab_types::load_config::{
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig,
};
use slab_types::runtime::DiffusionLoadOptions;
use slab_types::{Capability, ModelSource, RuntimeBackendId, RuntimeBackendLoadSpec};
use tonic::transport::Channel;
use tracing::{info, warn};
use uuid::Uuid;

use crate::context::{ModelState, WorkerState};
use crate::domain::models::{
    AcceptedOperation, AvailableModelsQuery, AvailableModelsView,
    CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
    ChatModelCapabilities, ChatModelOption, ChatModelSource, CreateModelCommand, DeletedModelView,
    DownloadModelCommand, ListModelsFilter, ManagedModelBackendId, ModelConfigDocument,
    ModelConfigFieldScope, ModelConfigFieldView, ModelConfigOrigin, ModelConfigPresetOption,
    ModelConfigSectionView, ModelConfigSelectionView, ModelConfigSourceArtifact,
    ModelConfigSourceSummary, ModelConfigValueType, ModelConfigVariantOption, ModelLoadCommand,
    ModelPackSelection, ModelSpec, ModelStatus, RuntimePresets, StoredModelConfig, TaskStatus,
    UnifiedModel, UnifiedModelKind, UnifiedModelStatus, UpdateModelCommand,
    UpdateModelConfigSelectionCommand, normalize_model_capabilities,
};
use crate::error::AppCoreError;
use crate::infra::db::{
    ModelConfigStateRecord, ModelConfigStateStore, ModelDownloadRecord, ModelDownloadStore,
    ModelStore, TaskRecord, UnifiedModelRecord,
};
use crate::infra::model_packs;
use crate::infra::rpc::{self, pb};
use crate::model_auto_unload::{ModelReplayPlan, build_model_load_request};

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;
type CloudProviderConfig = slab_types::settings::CloudProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelDownloadTaskInput {
    model_id: String,
    backend_id: String,
    repo_id: String,
    filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_cache_dir: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    artifacts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary_artifact_id: Option<String>,
}

fn validate_and_normalize_model_workers(
    backend_id: RuntimeBackendId,
    workers: u32,
    source: &'static str,
) -> Result<(u32, &'static str), AppCoreError> {
    if workers == 0 {
        return Err(AppCoreError::BadRequest("num_workers must be at least 1".into()));
    }

    if backend_id == RuntimeBackendId::GgmlDiffusion && workers > 1 {
        warn!(
            backend = %backend_id,
            requested_workers = workers,
            worker_source = source,
            "ggml.diffusion currently supports only one effective worker; clamping num_workers to 1 to avoid inconsistent per-worker model state"
        );
        return Ok((1, source));
    }

    Ok((workers, source))
}

fn primary_artifact_key<T>(files: &BTreeMap<String, T>) -> Option<String> {
    if files.contains_key("model") {
        return Some("model".to_owned());
    }
    if files.contains_key("diffusion_model") {
        return Some("diffusion_model".to_owned());
    }

    files.keys().next().cloned()
}

fn primary_materialized_artifact_path(config: &StoredModelConfig) -> Option<String> {
    primary_artifact_key(&config.materialized_artifacts)
        .and_then(|key| config.materialized_artifacts.get(&key).cloned())
}

#[derive(Debug, Clone)]
struct ResolvedModelLoadTarget {
    backend_id: RuntimeBackendId,
    model_path: String,
    model_id: Option<String>,
    pack_load_defaults: Option<slab_model_pack::ModelPackLoadDefaults>,
}

#[derive(Debug, Clone)]
struct ModelPackContext {
    path: std::path::PathBuf,
    resolved: slab_model_pack::ResolvedModelPack,
    persisted: Option<StoredModelConfig>,
}

#[derive(Debug, Clone)]
struct ResolvedModelPackSelectionView {
    explicit_selection: ModelPackSelection,
    effective_selection: ModelPackSelection,
    selected_preset: slab_model_pack::ResolvedPreset,
    warnings: Vec<String>,
    legacy_selection_to_import: Option<ModelPackSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ModelDownloadSourceKey {
    model_id: String,
    repo_id: String,
    filename: String,
}

#[derive(Clone)]
pub struct ModelService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl ModelService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self { model_state, worker_state }
    }

    pub async fn create_model(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        self.persist_model_definition(req).await
    }

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

    pub async fn get_model(&self, id: &str) -> Result<UnifiedModel, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;

        record.try_into().map_err(|e: String| AppCoreError::Internal(e))
    }

    pub async fn get_model_config_document(
        &self,
        id: &str,
    ) -> Result<ModelConfigDocument, AppCoreError> {
        let model = self.get_model(id).await?;
        resolve_local_backend_from_model(&model)?;

        let context = self.load_model_pack_context(id)?;
        let selection = self
            .resolve_model_pack_selection(id, &context.resolved, context.persisted.as_ref(), true)
            .await?;
        let command = build_model_command_from_pack_context(&context, &selection.selected_preset)?;
        let mut bridge = context.resolved.compile_runtime_bridge(&selection.selected_preset).map_err(
            |error| {
                AppCoreError::BadRequest(format!(
                    "failed to compile selected pack preset for config document: {error}"
                ))
            },
        )?;
        apply_materialized_source_to_bridge(&mut bridge, context.persisted.as_ref());
        let source_summary = build_model_config_source_summary(&bridge.model_spec.source);
        let selection_view = build_model_config_selection_view(
            &context.resolved,
            &selection.explicit_selection,
            &selection.effective_selection,
        );
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

    pub async fn update_model(
        &self,
        id: &str,
        req: UpdateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let existing_record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;

        let existing_model: UnifiedModel =
            existing_record.try_into().map_err(|e: String| AppCoreError::Internal(e))?;

        let next = CreateModelCommand {
            id: Some(existing_model.id),
            display_name: req.display_name.unwrap_or(existing_model.display_name),
            kind: req.kind.unwrap_or(existing_model.kind),
            backend_id: req.backend_id.or(existing_model.backend_id),
            capabilities: req.capabilities.or(Some(existing_model.capabilities)),
            status: Some(req.status.unwrap_or(existing_model.status)),
            spec: req.spec.unwrap_or(existing_model.spec),
            runtime_presets: req.runtime_presets.or(existing_model.runtime_presets),
        };

        self.persist_model_definition(next).await
    }

    pub async fn update_model_config_selection(
        &self,
        id: &str,
        req: UpdateModelConfigSelectionCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let current_model = self.get_model(id).await?;
        resolve_local_backend_from_model(&current_model)?;

        let context = self.load_model_pack_context(id)?;
        let explicit_selection = normalize_model_pack_selection(ModelPackSelection {
            preset_id: req.selected_preset_id,
            variant_id: req.selected_variant_id,
        });
        let selected_preset =
            resolve_selected_model_pack_preset(&context.resolved, &explicit_selection)?;
        let effective_selection = effective_model_pack_selection(
            &context.resolved,
            &explicit_selection,
            &selected_preset,
        );
        let mut command = build_model_command_from_pack_context(&context, &selected_preset)?;
        command.id = Some(current_model.id.clone());

        if same_model_download_source(&current_model.spec, &command.spec) {
            command.spec.local_path = current_model.spec.local_path.clone();
            command.status = Some(current_model.status.clone());
        } else if command.spec.repo_id.is_some() {
            command.spec.local_path = None;
            command.status = Some(UnifiedModelStatus::NotDownloaded);
        }

        let next_model = self.build_model_definition(command).await?;
        let stored_selection = selection_state_record_for_storage(
            id,
            &context.resolved,
            &explicit_selection,
            &effective_selection,
        );
        let stored_model = self.store_model_definition(next_model).await?;

        match stored_selection {
            Some(record) => self.model_state.store().upsert_model_config_state(record).await?,
            None => self.model_state.store().delete_model_config_state(id).await?,
        }

        Ok(stored_model)
    }

    pub async fn delete_model(&self, id: &str) -> Result<DeletedModelView, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;
        let model: UnifiedModel =
            record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;

        let _ = model_packs::delete_model_pack(self.model_config_dir(), id)?;
        if let Some(local_path) = model.spec.local_path.as_deref()
            && model_packs::is_model_pack_path(local_path)
        {
            let pack_path = std::path::Path::new(local_path);
            if pack_path.starts_with(self.model_config_dir()) {
                let _ = model_packs::delete_model_pack_at_path(pack_path)?;
            }
        }

        let _ = self.model_state.store().delete_model_config_state(id).await;
        self.model_state.store().delete_model(id).await?;
        self.model_state
            .auto_unload()
            .invalidate_model_replay(id, "model deleted from catalog")
            .await;
        Ok(DeletedModelView { id: id.to_owned(), status: "deleted".to_owned() })
    }

    pub async fn list_models(
        &self,
        query: ListModelsFilter,
    ) -> Result<Vec<UnifiedModel>, AppCoreError> {
        load_models_from_state(&self.model_state, query).await
    }

    pub async fn list_chat_models(&self) -> Result<Vec<ChatModelOption>, AppCoreError> {
        list_chat_models_from_state(&self.model_state).await
    }

    pub async fn load_model(&self, command: ModelLoadCommand) -> Result<ModelStatus, AppCoreError> {
        self.load_model_command("load_model", "loading model", command).await
    }

    pub async fn unload_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        let backend_id = resolve_unload_backend(&self.model_state, &command).await?;
        info!(backend = %backend_id, model_id = ?command.model_id, "unloading model");

        let (_, channel) = resolve_backend_channel(&self.model_state, backend_id)?;
        let response = rpc::client::unload_model(channel, backend_id, pb::ModelUnloadRequest {})
            .await
            .map_err(|error| map_grpc_model_error("unload_model", error))?;
        self.model_state.auto_unload().notify_model_unloaded(backend_id).await;

        decode_model_status(response)
    }

    pub async fn list_available_models(
        &self,
        query: AvailableModelsQuery,
    ) -> Result<AvailableModelsView, AppCoreError> {
        let repo_id = query.repo_id.clone();
        let files: Vec<String> = tokio::task::spawn_blocking(move || {
            let api = Api::new().map_err(|error| format!("hf-hub init failed: {error}"))?;
            let repo = api.model(repo_id);
            let info = repo.info().map_err(|error| format!("hf-hub info failed: {error}"))?;
            let names = info.siblings.into_iter().map(|item| item.rfilename).collect();
            Ok::<Vec<String>, String>(names)
        })
        .await
        .map_err(|error| AppCoreError::Internal(format!("spawn_blocking panicked: {error}")))?
        .map_err(AppCoreError::Internal)?;

        Ok(AvailableModelsView { repo_id: query.repo_id, files })
    }

    pub async fn switch_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        self.load_model_command("switch_model", "switching model", command).await
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

    pub async fn download_model(
        &self,
        req: DownloadModelCommand,
    ) -> Result<AcceptedOperation, AppCoreError> {
        let model_id = req.model_id.trim().to_owned();

        let configured_model_cache_dir = self.model_state.pmid().config().runtime.model_cache_dir;
        if let Some(dir) = &configured_model_cache_dir {
            validate_path("model_cache_dir", dir)?;
        }

        let record = self
            .model_state
            .store()
            .get_model(&model_id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;

        let model: UnifiedModel =
            record.try_into().map_err(|e: String| AppCoreError::Internal(e))?;

        let backend_id = resolve_local_backend_from_model(&model)?;

        let canonical_backend_id = backend_id.to_string();

        let (repo_id, download_artifacts, primary_artifact_id) =
            self.resolve_model_download_artifacts(&model).await?;
        let filename = primary_artifact_id
            .as_ref()
            .and_then(|artifact_id| download_artifacts.get(artifact_id))
            .cloned()
            .or_else(|| model.spec.filename.clone())
            .ok_or_else(|| {
                AppCoreError::BadRequest(format!(
                    "model {model_id} spec is missing filename required for download"
                ))
            })?;
        let download_key = model_download_source_key_from_parts(&model_id, &repo_id, &filename)
            .ok_or_else(|| {
                AppCoreError::BadRequest(format!(
                    "model {model_id} spec is missing repo_id or filename required for download"
                ))
            })?;

        self.model_state.store().reconcile_model_downloads().await?;
        if let Some(existing) = self
            .model_state
            .store()
            .get_active_model_download_for_source(
                &download_key.model_id,
                &download_key.repo_id,
                &download_key.filename,
            )
            .await?
        {
            info!(
                task_id = %existing.task_id,
                backend_id = %backend_id,
                model_id = %model_id,
                "reusing existing model download task"
            );
            return Ok(AcceptedOperation { operation_id: existing.task_id });
        }

        let input_data = serde_json::to_string(&ModelDownloadTaskInput {
            model_id: model.id,
            backend_id: canonical_backend_id,
            repo_id: repo_id.clone(),
            filename: filename.clone(),
            model_cache_dir: configured_model_cache_dir,
            artifacts: download_artifacts.clone(),
            primary_artifact_id: primary_artifact_id.clone(),
        })
        .map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize model download task input: {error}"))
        })?;

        let store = Arc::clone(self.model_state.store());
        let model_config_dir = self.model_config_dir().to_path_buf();
        let auto_unload = Arc::clone(self.model_state.auto_unload());
        let operation_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let insert_result = self
            .model_state
            .store()
            .insert_model_download_operation(
                TaskRecord {
                    id: operation_id.clone(),
                    task_type: "model_download".to_owned(),
                    status: TaskStatus::Pending,
                    model_id: Some(model_id.clone()),
                    input_data: Some(input_data.clone()),
                    result_data: None,
                    error_msg: None,
                    core_task_id: None,
                    created_at: now,
                    updated_at: now,
                },
                ModelDownloadRecord {
                    task_id: operation_id.clone(),
                    model_id: model_id.clone(),
                    repo_id: download_key.repo_id.clone(),
                    filename: download_key.filename.clone(),
                    status: TaskStatus::Pending,
                    error_msg: None,
                    created_at: now,
                    updated_at: now,
                },
            )
            .await;

        match insert_result {
            Ok(()) => {}
            Err(error) if is_model_download_conflict(&error) => {
                if let Some(existing) = self
                    .model_state
                    .store()
                    .get_active_model_download_for_source(
                        &download_key.model_id,
                        &download_key.repo_id,
                        &download_key.filename,
                    )
                    .await?
                {
                    info!(
                        task_id = %existing.task_id,
                        backend_id = %backend_id,
                        model_id = %model_id,
                        "reusing concurrently created model download task"
                    );
                    return Ok(AcceptedOperation { operation_id: existing.task_id });
                }

                return Err(error.into());
            }
            Err(error) => return Err(error.into()),
        }

        self.worker_state
            .spawn_existing_operation(operation_id.clone(), move |operation| async move {
                let operation_id = operation.id().to_owned();
                if let Err(error) = operation.mark_running().await {
                    warn!(task_id = %operation_id, error = %error, "failed to mark model download running");
                    let _ = persist_model_download_status(
                        &store,
                        &operation_id,
                        TaskStatus::Failed,
                        Some(&error.to_string()),
                    )
                    .await;
                    return;
                }

                if let Err(error) =
                    persist_model_download_status(&store, &operation_id, TaskStatus::Running, None)
                        .await
                {
                    warn!(task_id = %operation_id, error = %error, "failed to persist model download running status");
                }

                let input: ModelDownloadTaskInput = match serde_json::from_str(&input_data) {
                    Ok(value) => value,
                    Err(error) => {
                        warn!(task_id = %operation_id, error = %error, "invalid stored input_data for model_download task");
                        let message = format!("invalid stored input_data: {error}");
                        mark_model_download_failed(&operation, &store, &operation_id, &message).await;
                        return;
                    }
                };

                let model_id = input.model_id.trim().to_owned();
                let repo_id = input.repo_id.trim().to_owned();
                let filename = input.filename.trim().to_owned();
                let model_cache_dir = input
                    .model_cache_dir
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let download_artifacts = if input.artifacts.is_empty() {
                    let mut artifacts = BTreeMap::new();
                    artifacts.insert("model".to_owned(), filename.clone());
                    artifacts
                } else {
                    input.artifacts
                };
                let primary_artifact_id =
                    input.primary_artifact_id.or_else(|| primary_artifact_key(&download_artifacts));

                if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
                    warn!(task_id = %operation_id, "model_download task is missing model_id, repo_id, or filename");
                    mark_model_download_failed(
                        &operation,
                        &store,
                        &operation_id,
                        "missing model_id, repo_id, or filename in stored input_data",
                    )
                    .await;
                    return;
                }

                let result = tokio::task::spawn_blocking(move || {
                    let api = if let Some(dir) = model_cache_dir {
                        ApiBuilder::new()
                            .with_cache_dir(std::path::PathBuf::from(dir))
                            .build()
                            .map_err(|error| format!("hf-hub build failed: {error}"))?
                    } else {
                        Api::new().map_err(|error| format!("hf-hub init failed: {error}"))?
                    };
                    let repo = api.model(repo_id.clone());
                    let mut materialized_artifacts = BTreeMap::new();

                    for (artifact_id, artifact_file) in &download_artifacts {
                        let path = repo
                            .get(artifact_file)
                            .map_err(|error| format!("hf-hub download failed for {artifact_file}: {error}"))?;
                        materialized_artifacts
                            .insert(artifact_id.clone(), path.to_string_lossy().into_owned());
                    }

                    let local_path = primary_artifact_id
                        .as_ref()
                        .and_then(|artifact_id| materialized_artifacts.get(artifact_id))
                        .cloned()
                        .or_else(|| materialized_artifacts.values().next().cloned())
                        .ok_or_else(|| "hf-hub download produced no local artifacts".to_owned())?;

                    Ok::<(String, BTreeMap<String, String>), String>((local_path, materialized_artifacts))
                })
                .await;

                match result {
                    Ok(Ok((local_path, materialized_artifacts))) => {
                        if let Err(error) = store
                            .update_model_local_path(&model_id, &local_path, "ready")
                            .await
                        {
                            warn!(task_id = %operation_id, error = %error, "failed to persist downloaded model path");
                            let message =
                                format!("downloaded file but failed to persist path: {error}");
                            mark_model_download_failed(&operation, &store, &operation_id, &message)
                                .await;
                            return;
                        }

                        let updated_record = match store.get_model(&model_id).await {
                            Ok(Some(record)) => record,
                            Ok(None) => {
                                let message = format!(
                                    "downloaded file but model {model_id} no longer exists after update"
                                );
                                mark_model_download_failed(
                                    &operation,
                                    &store,
                                    &operation_id,
                                    &message,
                                )
                                .await;
                                return;
                            }
                            Err(error) => {
                                let message = format!(
                                    "downloaded file but failed to reload model {model_id}: {error}"
                                );
                                mark_model_download_failed(
                                    &operation,
                                    &store,
                                    &operation_id,
                                    &message,
                                )
                                .await;
                                return;
                            }
                        };

                        if let Err(error) = sync_model_pack_record(
                            &model_config_dir,
                            updated_record,
                            Some(materialized_artifacts),
                        )
                        {
                            warn!(task_id = %operation_id, error = %error, "failed to sync downloaded model pack");
                            let message =
                                format!("downloaded file but failed to sync model pack: {error}");
                            mark_model_download_failed(&operation, &store, &operation_id, &message)
                                .await;
                            return;
                        }

                        auto_unload
                            .invalidate_model_replay(
                                &model_id,
                                "model download updated catalog local_path",
                            )
                            .await;

                        let result_json =
                            serde_json::json!({ "local_path": local_path }).to_string();
                        if let Err(db_error) = operation.mark_succeeded(&result_json).await {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist model download success");
                        }
                        if let Err(error) =
                            persist_model_download_status(&store, &operation_id, TaskStatus::Succeeded, None)
                                .await
                        {
                            warn!(task_id = %operation_id, error = %error, "failed to persist model download success status");
                        }
                        info!(task_id = %operation_id, local_path = %local_path, "model download succeeded");
                    }
                    Ok(Err(error)) => {
                        warn!(task_id = %operation_id, error = %error, "model download failed");
                        mark_model_download_failed(&operation, &store, &operation_id, &error).await;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        warn!(task_id = %operation_id, error = %message, "model download task panicked");
                        mark_model_download_failed(&operation, &store, &operation_id, &message)
                            .await;
                    }
                }
            });

        info!(
            task_id = %operation_id,
            backend_id = %backend_id,
            model_id = %model_id,
            "model download task accepted"
        );

        Ok(AcceptedOperation { operation_id })
    }

    async fn resolve_model_download_artifacts(
        &self,
        model: &UnifiedModel,
    ) -> Result<(String, BTreeMap<String, String>, Option<String>), AppCoreError> {
        let repo_id = model.spec.repo_id.clone().ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model {} spec is missing repo_id required for download",
                model.id
            ))
        })?;
        let filename = model.spec.filename.clone().ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model {} spec is missing filename required for download",
                model.id
            ))
        })?;

        let pack_path = model_packs::model_pack_file_path(self.model_config_dir(), &model.id);
        if !pack_path.exists() {
            let mut artifacts = BTreeMap::new();
            artifacts.insert("model".to_owned(), filename);
            return Ok((repo_id, artifacts, Some("model".to_owned())));
        }

        let context = self.load_model_pack_context(&model.id)?;
        let selection = self
            .resolve_model_pack_selection(&model.id, &context.resolved, context.persisted.as_ref(), true)
            .await?;
        let bridge = context.resolved.compile_runtime_bridge(&selection.selected_preset).map_err(
            |error| {
                AppCoreError::BadRequest(format!(
                    "failed to compile selected pack preset for download plan: {error}"
                ))
            },
        )?;

        let ModelSource::HuggingFace { repo_id: source_repo_id, files, .. } = bridge.model_spec.source
        else {
            let mut artifacts = BTreeMap::new();
            artifacts.insert("model".to_owned(), filename);
            return Ok((repo_id, artifacts, Some("model".to_owned())));
        };

        let artifacts = files
            .into_iter()
            .map(|(artifact_id, path)| (artifact_id, path.to_string_lossy().into_owned()))
            .collect::<BTreeMap<_, _>>();
        let primary_artifact_id = primary_artifact_key(&artifacts);

        Ok((source_repo_id, artifacts, primary_artifact_id))
    }

    fn model_config_dir(&self) -> &std::path::Path {
        self.model_state.config().model_config_dir.as_path()
    }

    fn load_model_pack_context(&self, id: &str) -> Result<ModelPackContext, AppCoreError> {
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

    async fn build_selected_model_pack_command(
        &self,
        id: &str,
        persist_legacy_selection: bool,
    ) -> Result<(CreateModelCommand, Option<ModelConfigStateRecord>), AppCoreError> {
        let context = self.load_model_pack_context(id)?;
        if matches!(
            context.resolved.manifest.source.as_ref(),
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

    async fn resolve_model_pack_selection(
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
                preset_id: normalize_optional_text(record.selected_preset_id.clone()),
                variant_id: normalize_optional_text(record.selected_variant_id.clone()),
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

        if persist_legacy_selection && state_record.is_none() {
            if let Some(selection) = legacy_selection_to_import.as_ref() {
                self.model_state
                    .store()
                    .upsert_model_config_state(model_config_state_record(
                        model_id,
                        selection.preset_id.clone(),
                        selection.variant_id.clone(),
                    ))
                    .await?;
            }
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
            validate_and_normalize_model_workers(backend_id, workers, "model_pack")?
        } else {
            resolve_model_workers(&self.model_state, backend_id, None).await?
        };
        object.insert("num_workers".into(), Value::from(workers));

        if backend_id == RuntimeBackendId::GgmlLlama {
            if let Some(context_length) = bridge.load_defaults.context_length {
                object.insert("context_length".into(), Value::from(context_length));
            } else {
                let (context_length, source) =
                    resolve_llama_context_length(&self.model_state, backend_id).await?;
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
                resolve_diffusion_context_params(&self.model_state, backend_id).await?,
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
                        Some("Resolved diffusion artifact path.".into()),
                        ModelConfigValueType::Path,
                        json_property_or_null(resolved_load_spec, path),
                        diffusion_load_origin(bridge, path),
                    ));
                }
                load_fields.push(build_model_config_field(
                    "load.flash_attn",
                    ModelConfigFieldScope::Load,
                    "Flash Attention",
                    Some("Effective flash attention toggle for diffusion loads.".into()),
                    ModelConfigValueType::Boolean,
                    json_property_or_null(resolved_load_spec, "flash_attn"),
                    diffusion_load_origin(bridge, "flash_attn"),
                ));
                load_fields.push(build_model_config_field(
                    "load.vae_device",
                    ModelConfigFieldScope::Load,
                    "VAE Device",
                    Some("Device override for VAE execution.".into()),
                    ModelConfigValueType::String,
                    json_property_or_null(resolved_load_spec, "vae_device"),
                    diffusion_load_origin(bridge, "vae_device"),
                ));
                load_fields.push(build_model_config_field(
                    "load.clip_device",
                    ModelConfigFieldScope::Load,
                    "CLIP Device",
                    Some("Device override for CLIP execution.".into()),
                    ModelConfigValueType::String,
                    json_property_or_null(resolved_load_spec, "clip_device"),
                    diffusion_load_origin(bridge, "clip_device"),
                ));
                load_fields.push(build_model_config_field(
                    "load.offload_params_to_cpu",
                    ModelConfigFieldScope::Load,
                    "Offload Params To CPU",
                    Some("Whether diffusion parameters are offloaded to CPU.".into()),
                    ModelConfigValueType::Boolean,
                    json_property_or_null(resolved_load_spec, "offload_params_to_cpu"),
                    diffusion_load_origin(bridge, "offload_params_to_cpu"),
                ));
            }
            RuntimeBackendId::GgmlWhisper => {}
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

    async fn persist_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        self.persist_model_definition_with_options(req, true).await
    }

    async fn persist_model_definition_with_options(
        &self,
        req: CreateModelCommand,
        sync_model_pack: bool,
    ) -> Result<UnifiedModel, AppCoreError> {
        let model = self.build_model_definition(req).await?;
        if sync_model_pack {
            self.write_model_pack(&model)?;
        }

        self.store_model_definition(model).await
    }

    async fn store_model_definition(
        &self,
        model: UnifiedModel,
    ) -> Result<UnifiedModel, AppCoreError> {
        let record = model_to_record(&model)?;
        self.model_state.store().upsert_model(record).await?;
        self.model_state
            .auto_unload()
            .invalidate_model_replay(&model.id, "model definition upserted")
            .await;
        Ok(model)
    }

    async fn build_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, AppCoreError> {
        let id = normalize_required_text(
            req.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "id",
        )?;
        let display_name = normalize_required_text(req.display_name, "display_name")?;
        let (backend_id, spec) = canonicalize_model_spec(req.kind, req.backend_id, req.spec)?;
        let capabilities = normalize_model_capabilities(
            req.kind,
            backend_id,
            &display_name,
            &spec,
            req.capabilities,
        );
        let runtime_presets = canonicalize_runtime_presets(req.runtime_presets);
        let status = req.status.unwrap_or_else(|| default_status_for_kind(req.kind));

        let existing_record = self.model_state.store().get_model(&id).await?;
        let now = Utc::now();
        let created_at = existing_record.as_ref().map(|record| record.created_at).unwrap_or(now);

        Ok(UnifiedModel {
            id,
            display_name,
            kind: req.kind,
            backend_id,
            capabilities,
            status,
            spec,
            runtime_presets,
            created_at,
            updated_at: now,
        })
    }

    fn write_model_pack(&self, model: &UnifiedModel) -> Result<(), AppCoreError> {
        model_packs::write_persisted_model_pack(self.model_config_dir(), model)?;
        Ok(())
    }

    async fn load_model_command(
        &self,
        action: &'static str,
        log_message: &'static str,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        load_model_with_state(self.model_state.clone(), action, log_message, command).await
    }
}

pub(crate) async fn list_chat_models_from_state(
    state: &ModelState,
) -> Result<Vec<ChatModelOption>, AppCoreError> {
    let providers = load_cloud_provider_map_for_chat(state).await?;
    let records = load_models_from_state(
        state,
        ListModelsFilter { capability: Some(Capability::ChatGeneration) },
    )
    .await?;
    let mut items = Vec::new();

    for model in records {
        if let Some(item) = build_local_chat_model_option(&model) {
            items.push(item);
            continue;
        }

        if let Some(item) = build_cloud_chat_model_option(&providers, &model) {
            items.push(item);
        }
    }

    items.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });

    Ok(items)
}

async fn load_models_from_state(
    state: &ModelState,
    query: ListModelsFilter,
) -> Result<Vec<UnifiedModel>, AppCoreError> {
    state.store().reconcile_model_downloads().await?;

    let records = state.store().list_models().await?;
    let latest_downloads = load_latest_model_downloads_by_source(state).await?;
    let requested_capability = query.capability;
    let models = records
        .into_iter()
        .filter_map(|record| {
            record
                .try_into()
                .map(|mut model: UnifiedModel| {
                    model.status = effective_model_status(&model, &latest_downloads);
                    model
                })
                .map_err(|error: String| {
                    warn!(error = %error, "failed to deserialize model record; skipping");
                })
                .ok()
        })
        .filter(|model: &UnifiedModel| {
            requested_capability.is_none_or(|capability| model.capabilities.contains(&capability))
        })
        .collect();
    Ok(models)
}

async fn load_latest_model_downloads_by_source(
    state: &ModelState,
) -> Result<HashMap<ModelDownloadSourceKey, ModelDownloadRecord>, AppCoreError> {
    let mut latest = HashMap::new();

    for download in state.store().list_model_downloads().await? {
        let Some(key) = model_download_source_key_from_parts(
            &download.model_id,
            &download.repo_id,
            &download.filename,
        ) else {
            continue;
        };

        latest.entry(key).or_insert(download);
    }

    Ok(latest)
}

fn effective_model_status(
    model: &UnifiedModel,
    latest_downloads: &HashMap<ModelDownloadSourceKey, ModelDownloadRecord>,
) -> UnifiedModelStatus {
    if model.kind != UnifiedModelKind::Local {
        return model.status.clone();
    }

    let base_status = normalized_local_model_status(model);
    let Some(source_key) = model_download_source_key(model) else {
        return base_status;
    };

    let Some(download) = latest_downloads.get(&source_key) else {
        return base_status;
    };

    match download.status {
        TaskStatus::Pending | TaskStatus::Running => UnifiedModelStatus::Downloading,
        TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Interrupted => {
            UnifiedModelStatus::Error
        }
        TaskStatus::Succeeded => base_status,
    }
}

fn normalized_local_model_status(model: &UnifiedModel) -> UnifiedModelStatus {
    if model.spec.local_path.as_deref().map(str::trim).is_some_and(|value| !value.is_empty()) {
        return UnifiedModelStatus::Ready;
    }

    match &model.status {
        UnifiedModelStatus::Error => UnifiedModelStatus::Error,
        _ => UnifiedModelStatus::NotDownloaded,
    }
}

fn model_download_source_key(model: &UnifiedModel) -> Option<ModelDownloadSourceKey> {
    model_download_source_key_from_parts(
        &model.id,
        model.spec.repo_id.as_deref().unwrap_or_default(),
        model.spec.filename.as_deref().unwrap_or_default(),
    )
}

fn model_download_source_key_from_parts(
    model_id: &str,
    repo_id: &str,
    filename: &str,
) -> Option<ModelDownloadSourceKey> {
    let model_id = model_id.trim();
    let repo_id = repo_id.trim();
    let filename = filename.trim();
    if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
        return None;
    }

    Some(ModelDownloadSourceKey {
        model_id: model_id.to_owned(),
        repo_id: repo_id.to_owned(),
        filename: filename.to_owned(),
    })
}

fn is_model_download_conflict(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db_error) if db_error.message().contains("UNIQUE constraint failed"))
}

async fn persist_model_download_status(
    store: &Arc<crate::infra::db::AnyStore>,
    task_id: &str,
    status: TaskStatus,
    error_msg: Option<&str>,
) -> Result<(), AppCoreError> {
    store.update_model_download_status(task_id, status, error_msg).await?;
    Ok(())
}

async fn mark_model_download_failed(
    operation: &crate::context::worker_state::OperationContext,
    store: &Arc<crate::infra::db::AnyStore>,
    task_id: &str,
    message: &str,
) {
    if let Err(db_error) = operation.mark_failed(message).await {
        warn!(task_id = %task_id, error = %db_error, "failed to persist model download failure");
    }
    if let Err(error) =
        persist_model_download_status(store, task_id, TaskStatus::Failed, Some(message)).await
    {
        warn!(task_id = %task_id, error = %error, "failed to persist model download error status");
    }
}

async fn load_cloud_provider_map_for_chat(
    state: &ModelState,
) -> Result<BTreeMap<String, CloudProviderConfig>, AppCoreError> {
    Ok(state
        .pmid()
        .config()
        .chat
        .providers
        .into_iter()
        .map(|provider| (provider.id.clone(), provider))
        .collect())
}

fn is_cloud_catalog_model_for_chat(model: &UnifiedModel) -> bool {
    model.kind == UnifiedModelKind::Cloud
        && model.capabilities.contains(&Capability::ChatGeneration)
}

fn is_local_chat_model(model: &UnifiedModel) -> bool {
    model.kind == UnifiedModelKind::Local
        && model.capabilities.contains(&Capability::ChatGeneration)
}

fn referenced_cloud_provider_id(model: &UnifiedModel) -> Option<String> {
    model
        .spec
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn local_chat_model_downloaded(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Ready) && model.spec.local_path.is_some()
}

fn local_chat_model_pending(model: &UnifiedModel) -> bool {
    matches!(model.status, UnifiedModelStatus::Downloading)
}

fn build_local_chat_model_option(model: &UnifiedModel) -> Option<ChatModelOption> {
    if !is_local_chat_model(model) {
        return None;
    }

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Local,
        downloaded: local_chat_model_downloaded(model),
        pending: local_chat_model_pending(model),
        capabilities: ChatModelCapabilities::local(),
        backend_id: model.backend_id.clone(),
        provider_id: None,
        provider_name: None,
    })
}

fn build_cloud_chat_model_option(
    providers: &BTreeMap<String, CloudProviderConfig>,
    model: &UnifiedModel,
) -> Option<ChatModelOption> {
    if !is_cloud_catalog_model_for_chat(model) {
        return None;
    }

    let provider_id = referenced_cloud_provider_id(model)?;
    let remote_model_id =
        model.spec.remote_model_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if remote_model_id.is_none() {
        warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model is missing remote_model_id; hiding from chat picker"
        );
        return None;
    }
    let Some(provider) = providers.get(&provider_id) else {
        warn!(
            model_id = %model.id,
            provider_id = %provider_id,
            "cloud model references unknown provider; hiding from chat picker"
        );
        return None;
    };

    Some(ChatModelOption {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        source: ChatModelSource::Cloud,
        downloaded: true,
        pending: false,
        capabilities: ChatModelCapabilities::cloud(),
        backend_id: None,
        provider_id: Some(provider_id),
        provider_name: Some(provider.name.clone()),
    })
}

fn canonicalize_model_spec(
    kind: UnifiedModelKind,
    backend_id: Option<ManagedModelBackendId>,
    mut spec: ModelSpec,
) -> Result<(Option<ManagedModelBackendId>, ModelSpec), AppCoreError> {
    spec.provider_id = normalize_optional_text(spec.provider_id);
    spec.remote_model_id = normalize_optional_text(spec.remote_model_id);
    spec.repo_id = normalize_optional_text(spec.repo_id);
    spec.filename = normalize_optional_text(spec.filename);
    spec.local_path = normalize_optional_text(spec.local_path);
    spec.chat_template = normalize_optional_text(spec.chat_template);

    match kind {
        UnifiedModelKind::Cloud => {
            spec.repo_id = None;
            spec.filename = None;
            spec.local_path = None;
            spec.chat_template = None;

            if spec.provider_id.is_none() {
                return Err(AppCoreError::BadRequest(
                    "cloud models must set spec.provider_id to a configured providers.registry entry"
                        .into(),
                ));
            }
            if spec.remote_model_id.is_none() {
                return Err(AppCoreError::BadRequest(
                    "cloud models must set spec.remote_model_id".into(),
                ));
            }

            Ok((None, spec))
        }
        UnifiedModelKind::Local => {
            spec.provider_id = None;
            spec.remote_model_id = None;

            let backend_id = backend_id.ok_or_else(|| {
                AppCoreError::BadRequest("local models must set backend_id".into())
            })?;

            Ok((Some(backend_id), spec))
        }
    }
}

fn canonicalize_runtime_presets(
    runtime_presets: Option<crate::domain::models::RuntimePresets>,
) -> Option<crate::domain::models::RuntimePresets> {
    runtime_presets.filter(|presets| presets.temperature.is_some() || presets.top_p.is_some())
}

fn default_status_for_kind(kind: UnifiedModelKind) -> UnifiedModelStatus {
    match kind {
        UnifiedModelKind::Cloud => UnifiedModelStatus::Ready,
        UnifiedModelKind::Local => UnifiedModelStatus::NotDownloaded,
    }
}

fn normalize_required_text(value: String, label: &str) -> Result<String, AppCoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppCoreError::BadRequest(format!("{label} must not be empty")));
    }
    Ok(trimmed.to_owned())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) }
    })
}

fn model_to_record(model: &UnifiedModel) -> Result<UnifiedModelRecord, AppCoreError> {
    let spec_json = serde_json::to_string(&model.spec)
        .map_err(|error| AppCoreError::Internal(format!("failed to serialize spec: {error}")))?;
    let capabilities_json = serde_json::to_string(&model.capabilities).map_err(|error| {
        AppCoreError::Internal(format!("failed to serialize capabilities: {error}"))
    })?;
    let runtime_presets_json =
        model.runtime_presets.as_ref().map(serde_json::to_string).transpose().map_err(|error| {
            AppCoreError::Internal(format!("failed to serialize runtime_presets: {error}"))
        })?;

    Ok(UnifiedModelRecord {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        provider: legacy_provider_value(model),
        kind: model.kind.as_str().to_owned(),
        backend_id: model.backend_id.map(|backend_id| backend_id.to_string()),
        capabilities: capabilities_json,
        status: model.status.as_str().to_owned(),
        spec: spec_json,
        runtime_presets: runtime_presets_json,
        config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
        config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn legacy_provider_value(model: &UnifiedModel) -> String {
    match model.kind {
        UnifiedModelKind::Local => model
            .backend_id
            .map(|backend_id| format!("local.{backend_id}"))
            .unwrap_or_else(|| "local".to_owned()),
        UnifiedModelKind::Cloud => model
            .spec
            .provider_id
            .as_deref()
            .map(|provider_id| format!("cloud.{provider_id}"))
            .unwrap_or_else(|| "cloud".to_owned()),
    }
}

fn sync_model_pack_record(
    config_dir: &std::path::Path,
    record: UnifiedModelRecord,
    materialized_artifacts: Option<BTreeMap<String, String>>,
) -> Result<(), AppCoreError> {
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    let mut config: StoredModelConfig = model.into();
    if let Some(materialized_artifacts) = materialized_artifacts {
        config.materialized_artifacts = materialized_artifacts;
        if config.spec.local_path.is_none() {
            config.spec.local_path = primary_materialized_artifact_path(&config);
        }
    } else {
        let existing_path = model_packs::model_pack_file_path(config_dir, &config.id);
        if existing_path.exists()
            && let Some(existing) = model_packs::read_persisted_model_config_from_pack(&existing_path)?
        {
            config.materialized_artifacts = existing.materialized_artifacts;
        }
    }

    model_packs::write_persisted_model_pack_from_config(config_dir, &config)?;
    Ok(())
}

fn default_model_pack_selection(
    resolved: &slab_model_pack::ResolvedModelPack,
) -> ModelPackSelection {
    let default_preset = resolved.default_preset();

    ModelPackSelection {
        preset_id: resolved.default_preset_id.clone(),
        variant_id: default_preset
            .and_then(|preset| non_empty_variant_id(&preset.variant.document.id)),
    }
}

fn non_empty_variant_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn normalize_model_pack_selection(selection: ModelPackSelection) -> ModelPackSelection {
    ModelPackSelection {
        preset_id: normalize_optional_text(selection.preset_id),
        variant_id: normalize_optional_text(selection.variant_id),
    }
}

fn resolve_selected_model_pack_preset(
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

fn build_local_model_command_from_pack_preset(
    manifest: &slab_model_pack::ModelPackManifest,
    resolved: &slab_model_pack::ResolvedModelPack,
    preset: &slab_model_pack::ResolvedPreset,
) -> Result<CreateModelCommand, AppCoreError> {
    let bridge = resolved.compile_runtime_bridge(preset).map_err(|error| {
        AppCoreError::BadRequest(format!("failed to compile selected pack preset: {error}"))
    })?;
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
            slab_model_pack::PackModelStatus::NotDownloaded => UnifiedModelStatus::NotDownloaded,
            slab_model_pack::PackModelStatus::Downloading => UnifiedModelStatus::Downloading,
            slab_model_pack::PackModelStatus::Error => UnifiedModelStatus::Error,
        })
        .unwrap_or_else(|| match bridge.model_spec.source {
            slab_types::ModelSource::HuggingFace { .. } => UnifiedModelStatus::NotDownloaded,
            _ => UnifiedModelStatus::Ready,
        });
    let runtime_presets = manifest
        .runtime_presets
        .as_ref()
        .and_then(|presets| {
            (presets.temperature.is_some() || presets.top_p.is_some()).then_some(RuntimePresets {
                temperature: presets.temperature,
                top_p: presets.top_p,
            })
        })
        .or_else(|| {
            let temperature = bridge
                .inference_defaults
                .get("temperature")
                .and_then(|value| value.as_f64().map(|value| value as f32));
            let top_p = bridge
                .inference_defaults
                .get("top_p")
                .and_then(|value| value.as_f64().map(|value| value as f32));
            (temperature.is_some() || top_p.is_some())
                .then_some(RuntimePresets { temperature, top_p })
        });
    let (repo_id, filename, local_path) =
        source_preview_from_model_source(&bridge.model_spec.source);
    let allow_local_path_fallback = repo_id.is_none();

    Ok(CreateModelCommand {
        id: Some(manifest.id.clone()),
        display_name: manifest.label.clone(),
        kind: UnifiedModelKind::Local,
        backend_id: Some(backend_id),
        capabilities: Some(manifest.capabilities.clone()),
        status: Some(status),
        spec: ModelSpec {
            pricing: manifest.pricing.as_ref().map(|pricing| crate::domain::models::Pricing {
                input: pricing.input,
                output: pricing.output,
            }),
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
            chat_template: bridge.load_defaults.chat_template.clone(),
            ..Default::default()
        },
        runtime_presets,
    })
}

fn source_preview_from_pack_source(
    source: Option<&slab_model_pack::PackSource>,
) -> (Option<String>, Option<String>, Option<String>) {
    match source {
        Some(slab_model_pack::PackSource::HuggingFace { repo_id, files, .. }) => (
            Some(repo_id.clone()),
            files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone()),
            None,
        ),
        Some(slab_model_pack::PackSource::LocalPath { path }) => (None, None, Some(path.clone())),
        Some(slab_model_pack::PackSource::LocalFiles { files }) => (
            None,
            None,
            files
                .iter()
                .find(|file| file.id == "model")
                .or_else(|| files.first())
                .map(|file| file.path.clone()),
        ),
        Some(slab_model_pack::PackSource::Cloud { .. }) | None => (None, None, None),
    }
}

fn source_preview_from_model_source(
    source: &slab_types::ModelSource,
) -> (Option<String>, Option<String>, Option<String>) {
    match source {
        slab_types::ModelSource::HuggingFace { repo_id, files, .. } => (
            Some(repo_id.clone()),
            files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
            None,
        ),
        slab_types::ModelSource::LocalPath { path } => {
            (None, None, Some(path.to_string_lossy().into_owned()))
        }
        slab_types::ModelSource::LocalArtifacts { files } => (
            None,
            None,
            files
                .get("model")
                .or_else(|| files.values().next())
                .map(|path| path.to_string_lossy().into_owned()),
        ),
        _ => (None, None, None),
    }
}

fn materialized_model_source(
    source: &slab_types::ModelSource,
    persisted: Option<&StoredModelConfig>,
) -> slab_types::ModelSource {
    let Some(persisted) = persisted else {
        return source.clone();
    };
    let (repo_id, filename, local_path) = source_preview_from_model_source(source);
    let projected_spec = ModelSpec {
        repo_id,
        filename,
        local_path,
        ..Default::default()
    };
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

    let Some(local_path) = persisted
        .spec
        .local_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
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

fn apply_materialized_source_to_bridge(
    bridge: &mut slab_model_pack::ModelPackRuntimeBridge,
    persisted: Option<&StoredModelConfig>,
) {
    bridge.model_spec.source = materialized_model_source(&bridge.model_spec.source, persisted);
}

fn same_model_download_source(current: &ModelSpec, next: &ModelSpec) -> bool {
    match (current.repo_id.as_deref(), next.repo_id.as_deref()) {
        (Some(_), Some(_)) => current.repo_id == next.repo_id && current.filename == next.filename,
        (None, None) => current.local_path == next.local_path,
        _ => false,
    }
}

fn build_model_command_from_pack_context(
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
    if same_model_download_source(&persisted.spec, &command.spec) {
        command.spec.local_path =
            persisted.spec.local_path.clone().or_else(|| primary_materialized_artifact_path(persisted));
        if let Some(status) = persisted.status.clone() {
            command.status = Some(status);
        }
    }
}

fn resolve_effective_model_pack_selection(
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

fn effective_model_pack_selection(
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

fn selection_state_record_for_storage(
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

fn model_config_state_record(
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

fn build_model_config_selection_view(
    resolved: &slab_model_pack::ResolvedModelPack,
    explicit_selection: &ModelPackSelection,
    effective_selection: &ModelPackSelection,
) -> ModelConfigSelectionView {
    let default_selection = default_model_pack_selection(resolved);
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
                .or_else(|| non_empty_variant_id(&preset.variant.document.id)),
            is_default: resolved.default_preset_id.as_deref() == Some(preset.document.id.as_str()),
        })
        .collect();
    let variants = resolved
        .variants
        .values()
        .map(|variant| {
            let (repo_id, filename, local_path) =
                source_preview_from_pack_source(variant.effective_source.as_ref());
            ModelConfigVariantOption {
                id: variant.document.id.clone(),
                label: variant.document.label.clone(),
                description: variant.document.description.clone(),
                repo_id,
                filename,
                local_path,
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
    if selected_preset.variant.document.source.is_some()
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

fn validate_path(label: &str, path: &str) -> Result<(), AppCoreError> {
    if path.is_empty() {
        return Err(AppCoreError::BadRequest(format!("{label} must not be empty")));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(AppCoreError::BadRequest(format!(
            "{label} must be an absolute path (got: {path})"
        )));
    }
    let has_traversal = std::path::Path::new(path)
        .components()
        .any(|component| component == std::path::Component::ParentDir);
    if has_traversal {
        return Err(AppCoreError::BadRequest(format!("{label} must not contain '..' components")));
    }
    Ok(())
}

fn validate_existing_model_file(path: &str) -> Result<(), AppCoreError> {
    let model_path = std::path::Path::new(path);
    if !model_path.exists() {
        return Err(AppCoreError::BadRequest(format!("model_path does not exist: {path}")));
    }
    if !model_path.is_file() {
        return Err(AppCoreError::BadRequest(format!("model_path is not a file: {path}")));
    }
    Ok(())
}

fn resolve_backend_channel(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<(RuntimeBackendId, Channel), AppCoreError> {
    let canonical_backend = backend_id.to_string();
    let channel = state.grpc().backend_channel(backend_id).ok_or_else(|| {
        AppCoreError::BackendNotReady(format!(
            "{canonical_backend} gRPC endpoint is not configured"
        ))
    })?;
    Ok((backend_id, channel))
}

async fn resolve_model_workers(
    state: &ModelState,
    backend_id: RuntimeBackendId,
    requested_workers: Option<u32>,
) -> Result<(u32, &'static str), AppCoreError> {
    if let Some(workers) = requested_workers {
        return validate_and_normalize_model_workers(backend_id, workers, "request");
    }

    let configured_workers = {
        let config = state.pmid().config();
        match backend_id {
            RuntimeBackendId::GgmlLlama => Some(config.runtime.llama.num_workers),
            RuntimeBackendId::GgmlWhisper => Some(config.runtime.whisper.num_workers),
            RuntimeBackendId::GgmlDiffusion => Some(config.runtime.diffusion.num_workers),
            _ => None,
        }
    };
    let Some(workers) = configured_workers else {
        return validate_and_normalize_model_workers(
            backend_id,
            DEFAULT_MODEL_NUM_WORKERS,
            "default",
        );
    };
    validate_and_normalize_model_workers(backend_id, workers, "settings")
}

async fn resolve_llama_context_length(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<(u32, &'static str), AppCoreError> {
    if backend_id != RuntimeBackendId::GgmlLlama {
        return Ok((0, "not_applicable"));
    }

    let configured = state.pmid().config().runtime.llama.context_length;
    let Some(context_length) = configured else {
        return Ok((0, "default"));
    };
    Ok((context_length, "settings"))
}

async fn resolve_diffusion_context_params(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<Option<DiffusionLoadOptions>, AppCoreError> {
    if backend_id != RuntimeBackendId::GgmlDiffusion {
        return Ok(None);
    }

    let config = state.pmid().config();
    let paths = config.diffusion.paths;
    let performance = config.diffusion.performance;

    Ok(Some(DiffusionLoadOptions {
        diffusion_model_path: paths.model.map(PathBuf::from),
        vae_path: paths.vae.map(PathBuf::from),
        taesd_path: paths.taesd.map(PathBuf::from),
        lora_model_dir: paths.lora_model_dir.map(PathBuf::from),
        clip_l_path: paths.clip_l.map(PathBuf::from),
        clip_g_path: paths.clip_g.map(PathBuf::from),
        t5xxl_path: paths.t5xxl.map(PathBuf::from),
        flash_attn: performance.flash_attn,
        vae_device: performance.vae_device,
        clip_device: performance.clip_device,
        offload_params_to_cpu: performance.offload_params_to_cpu,
    }))
}

fn grpc_status_message(status: &tonic::Status) -> String {
    let message = status.message().trim();
    if !message.is_empty() {
        return message.to_owned();
    }
    status.to_string()
}

fn decode_model_status(response: pb::ModelStatusResponse) -> Result<ModelStatus, AppCoreError> {
    let status = convert::decode_model_status_response(&response).map_err(|error| {
        AppCoreError::Internal(format!("invalid model status response from runtime: {error}"))
    })?;

    Ok(ModelStatus { backend: status.backend.to_string(), status: status.status })
}

fn map_grpc_model_error(action: &str, err: anyhow::Error) -> AppCoreError {
    if let Some(detail) = rpc::client::transient_runtime_detail(&err) {
        return AppCoreError::BackendNotReady(detail);
    }

    let grpc_status = err.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>());

    if let Some(status) = grpc_status {
        let detail = grpc_status_message(status);
        return match status.code() {
            tonic::Code::InvalidArgument
            | tonic::Code::FailedPrecondition
            | tonic::Code::ResourceExhausted => AppCoreError::BadRequest(detail),
            tonic::Code::NotFound => AppCoreError::NotFound(detail),
            tonic::Code::Unavailable => AppCoreError::BackendNotReady(detail),
            _ => AppCoreError::Internal(format!("grpc {action} failed: {err:#}")),
        };
    }

    AppCoreError::Internal(format!("grpc {action} failed: {err:#}"))
}

async fn load_model_with_state(
    state: ModelState,
    action: &'static str,
    log_message: &'static str,
    command: ModelLoadCommand,
) -> Result<ModelStatus, AppCoreError> {
    let resolved_target = resolve_model_load_target(&state, &command).await?;

    validate_path("model_path", &resolved_target.model_path)?;
    validate_existing_model_file(&resolved_target.model_path)?;

    let (_, channel) = resolve_backend_channel(&state, resolved_target.backend_id)?;
    let (num_workers, worker_source) = if let Some(workers) = command.num_workers {
        validate_and_normalize_model_workers(resolved_target.backend_id, workers, "request")?
    } else if let Some(workers) =
        resolved_target.pack_load_defaults.as_ref().and_then(|defaults| defaults.num_workers)
    {
        validate_and_normalize_model_workers(resolved_target.backend_id, workers, "model_pack")?
    } else {
        resolve_model_workers(&state, resolved_target.backend_id, None).await?
    };
    let (context_length, context_source) =
        resolve_llama_context_length(&state, resolved_target.backend_id).await?;
    let diffusion = if let Some(defaults) = resolved_target.pack_load_defaults.as_ref() {
        model_packs::merge_diffusion_load_defaults(
            defaults.diffusion.clone(),
            resolve_diffusion_context_params(&state, resolved_target.backend_id).await?,
        )
    } else {
        resolve_diffusion_context_params(&state, resolved_target.backend_id).await?
    };

    info!(
        backend = %resolved_target.backend_id,
        model_id = ?resolved_target.model_id,
        model_path = %resolved_target.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "{log_message}"
    );

    let load_spec = build_backend_load_spec(
        resolved_target.backend_id,
        &resolved_target.model_path,
        num_workers,
        context_length,
        resolved_target
            .pack_load_defaults
            .as_ref()
            .and_then(|defaults| defaults.chat_template.clone()),
        diffusion,
    )?;
    let grpc_req = build_model_load_request(&load_spec);
    let response = rpc::client::load_model(channel, resolved_target.backend_id, grpc_req)
        .await
        .map_err(|error| map_grpc_model_error(action, error))?;
    state
        .auto_unload()
        .notify_model_loaded(ModelReplayPlan {
            backend_id: resolved_target.backend_id,
            model_id: resolved_target.model_id,
            load_spec,
        })
        .await;

    decode_model_status(response)
}

fn build_backend_load_spec(
    backend_id: RuntimeBackendId,
    model_path: &str,
    num_workers: u32,
    context_length: u32,
    chat_template: Option<String>,
    diffusion: Option<DiffusionLoadOptions>,
) -> Result<RuntimeBackendLoadSpec, AppCoreError> {
    let model_path = PathBuf::from(model_path);

    match backend_id {
        RuntimeBackendId::GgmlLlama => Ok(RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
            model_path,
            num_workers: usize::try_from(num_workers).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to convert num_workers into usize for ggml.llama: {error}"
                ))
            })?,
            context_length: (context_length > 0).then_some(context_length),
            chat_template,
        })),
        RuntimeBackendId::GgmlWhisper => {
            Ok(RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig { model_path }))
        }
        RuntimeBackendId::GgmlDiffusion => {
            let diffusion = diffusion.unwrap_or_default();
            Ok(RuntimeBackendLoadSpec::GgmlDiffusion(GgmlDiffusionLoadConfig {
                model_path,
                diffusion_model_path: diffusion.diffusion_model_path,
                vae_path: diffusion.vae_path,
                taesd_path: diffusion.taesd_path,
                clip_l_path: diffusion.clip_l_path,
                clip_g_path: diffusion.clip_g_path,
                t5xxl_path: diffusion.t5xxl_path,
                clip_vision_path: None,
                control_net_path: None,
                flash_attn: diffusion.flash_attn,
                vae_device: (!diffusion.vae_device.is_empty()).then_some(diffusion.vae_device),
                clip_device: (!diffusion.clip_device.is_empty()).then_some(diffusion.clip_device),
                offload_params_to_cpu: diffusion.offload_params_to_cpu,
                enable_mmap: false,
                n_threads: None,
            }))
        }
        other => Err(AppCoreError::Internal(format!(
            "unsupported backend for managed model load spec assembly: {}",
            other.canonical_id()
        ))),
    }
}

async fn resolve_model_load_target(
    state: &ModelState,
    command: &ModelLoadCommand,
) -> Result<ResolvedModelLoadTarget, AppCoreError> {
    if let Some(model_id) =
        command.model_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        let model = resolve_local_catalog_model(state, model_id).await?;
        let backend_id = resolve_local_backend_from_model(&model)?;
        let model_path = resolve_local_model_path(&model)?;
        if model_packs::is_model_pack_path(&model_path) {
            let pack_target = build_selected_model_pack_load_target(
                state,
                &model.id,
                std::path::Path::new(&model_path),
            )
            .await?;
            if pack_target.backend_id != backend_id {
                return Err(AppCoreError::BadRequest(format!(
                    "model '{}' pack backend '{}' does not match catalog backend '{}'",
                    model.id, pack_target.backend_id, backend_id
                )));
            }
            return Ok(ResolvedModelLoadTarget {
                backend_id,
                model_path: pack_target.model_path,
                model_id: Some(model.id),
                pack_load_defaults: Some(pack_target.load_defaults),
            });
        }
        return Ok(ResolvedModelLoadTarget {
            backend_id,
            model_path,
            model_id: Some(model.id),
            pack_load_defaults: None,
        });
    }

    let backend_id = command
        .backend_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("backend_id is required when model_id is not provided".into())
        })?;
    let model_path = command
        .model_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("model_path is required when model_id is not provided".into())
        })?
        .to_owned();
    let backend_id: RuntimeBackendId = backend_id
        .parse()
        .map_err(|_| AppCoreError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let (backend_id, _) = resolve_backend_channel(state, backend_id)?;

    if model_packs::is_model_pack_path(&model_path) {
        let pack_target =
            model_packs::build_model_pack_load_target(std::path::Path::new(&model_path))?;
        if pack_target.backend_id != backend_id {
            return Err(AppCoreError::BadRequest(format!(
                "model pack backend '{}' does not match requested backend '{}'",
                pack_target.backend_id, backend_id
            )));
        }
        return Ok(ResolvedModelLoadTarget {
            backend_id,
            model_path: pack_target.model_path,
            model_id: None,
            pack_load_defaults: Some(pack_target.load_defaults),
        });
    }

    Ok(ResolvedModelLoadTarget { backend_id, model_path, model_id: None, pack_load_defaults: None })
}

async fn build_selected_model_pack_load_target(
    state: &ModelState,
    model_id: &str,
    pack_path: &std::path::Path,
) -> Result<model_packs::ModelPackLoadTarget, AppCoreError> {
    let pack = model_packs::open_model_pack(pack_path)?;
    let resolved = pack.resolve().map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to resolve model pack '{}': {error}",
            pack_path.display()
        ))
    })?;
    let persisted = model_packs::read_persisted_model_config_from_pack(pack_path)?;
    let state_record = state.store().get_model_config_state(model_id).await?;
    let legacy_selection = persisted
        .as_ref()
        .and_then(|config| config.pack_selection.clone())
        .map(normalize_model_pack_selection);
    let explicit_selection = if let Some(record) = state_record.as_ref() {
        ModelPackSelection {
            preset_id: normalize_optional_text(record.selected_preset_id.clone()),
            variant_id: normalize_optional_text(record.selected_variant_id.clone()),
        }
    } else {
        legacy_selection.clone().unwrap_or_default()
    };
    let (effective_selection, selected_preset, _) =
        resolve_effective_model_pack_selection(&resolved, &explicit_selection)?;

    if state_record.is_none() {
        if let Some(record) = selection_state_record_for_storage(
            model_id,
            &resolved,
            &legacy_selection.unwrap_or_default(),
            &effective_selection,
        ) {
            let _ = state.store().upsert_model_config_state(record).await;
        }
    }

    let mut bridge = resolved.compile_runtime_bridge(&selected_preset).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to compile selected pack preset for load target: {error}"
        ))
    })?;
    apply_materialized_source_to_bridge(&mut bridge, persisted.as_ref());
    let preset_id = effective_selection
        .preset_id
        .clone()
        .unwrap_or_else(|| selected_preset.document.id.clone());
    let load_spec = bridge.runtime_load_spec(&preset_id).map_err(|error| match error {
        slab_model_pack::ModelPackError::NonMaterializedSource { .. } => AppCoreError::BadRequest(
            format!(
                "model pack '{}' points to a remote source and must be downloaded from the model catalog before loading",
                pack_path.display()
            ),
        ),
        other => AppCoreError::BadRequest(format!(
            "failed to build selected model pack load target '{}': {other}",
            pack_path.display()
        )),
    })?;

    Ok(model_packs::ModelPackLoadTarget {
        backend_id: bridge.backend,
        model_path: load_spec.model_path.to_string_lossy().into_owned(),
        load_defaults: bridge.load_defaults,
    })
}

async fn resolve_unload_backend(
    state: &ModelState,
    command: &ModelLoadCommand,
) -> Result<RuntimeBackendId, AppCoreError> {
    if let Some(model_id) =
        command.model_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        let model = resolve_local_catalog_model(state, model_id).await?;
        let backend_id = resolve_local_backend_from_model(&model)?;
        let (backend_id, _) = resolve_backend_channel(state, backend_id)?;
        return Ok(backend_id);
    }

    let backend_id = command
        .backend_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("backend_id is required when model_id is not provided".into())
        })?;
    let backend_id: RuntimeBackendId = backend_id
        .parse()
        .map_err(|_| AppCoreError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let (backend_id, _) = resolve_backend_channel(state, backend_id)?;
    Ok(backend_id)
}

async fn resolve_local_catalog_model(
    state: &ModelState,
    model_id: &str,
) -> Result<UnifiedModel, AppCoreError> {
    let record = state
        .store()
        .get_model(model_id)
        .await?
        .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    resolve_local_backend_from_model(&model)?;
    Ok(model)
}

fn resolve_local_backend_from_model(
    model: &UnifiedModel,
) -> Result<RuntimeBackendId, AppCoreError> {
    if model.kind != UnifiedModelKind::Local {
        return Err(AppCoreError::BadRequest(format!(
            "model '{}' is '{}' and cannot be managed with local runtime lifecycle endpoints",
            model.id,
            model.kind.as_str()
        )));
    }

    let backend_id = model.backend_id.ok_or_else(|| {
        AppCoreError::BadRequest(format!("model '{}' is local but is missing backend_id", model.id))
    })?;

    Ok(backend_id.into())
}

fn resolve_local_model_path(model: &UnifiedModel) -> Result<String, AppCoreError> {
    model
        .spec
        .local_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model '{}' has no local_path yet; download it before loading",
                model.id
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        CloudProviderConfig, build_cloud_chat_model_option, build_local_chat_model_option,
        canonicalize_model_spec, canonicalize_runtime_presets, map_grpc_model_error,
        normalize_required_text, validate_and_normalize_model_workers,
    };
    use crate::domain::models::{
        ChatModelSource, ManagedModelBackendId, ModelSpec, RuntimePresets, UnifiedModel,
        UnifiedModelKind, UnifiedModelStatus, default_model_capabilities,
    };
    use crate::error::AppCoreError;
    use chrono::Utc;
    use slab_types::RuntimeBackendId;
    use std::collections::BTreeMap;

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
                filename: Some("qwen3-8b.gguf".into()),
                local_path: Some("C:/models/qwen3-8b.gguf".into()),
                chat_template: Some("chatml".into()),
                ..ModelSpec::default()
            },
        )
        .expect("cloud spec");

        assert!(spec.repo_id.is_none());
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
            Some(ManagedModelBackendId::GgmlLlama),
            ModelSpec {
                provider_id: Some("openai-main".into()),
                remote_model_id: Some("gpt-4.1-mini".into()),
                ..ModelSpec::default()
            },
        )
        .expect("local spec");

        assert_eq!(backend_id, Some(ManagedModelBackendId::GgmlLlama));
        assert!(spec.provider_id.is_none());
        assert!(spec.remote_model_id.is_none());
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
        assert_eq!(option.backend_id, Some(ManagedModelBackendId::GgmlLlama));
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
            CloudProviderConfig {
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
