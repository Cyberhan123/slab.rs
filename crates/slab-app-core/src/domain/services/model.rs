use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use hf_hub::api::sync::{Api, ApiBuilder};
use slab_proto::convert;
use slab_types::load_config::{GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig};
use slab_types::{Capability, RuntimeBackendId, RuntimeBackendLoadSpec};
use slab_types::runtime::DiffusionLoadOptions;
use tonic::transport::Channel;
use tracing::{info, warn};

use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, AvailableModelsQuery, AvailableModelsView,
    CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
    ChatModelCapabilities, ChatModelOption, ChatModelSource, CreateModelCommand,
    DeletedModelView, DownloadModelCommand, ListModelsFilter, ModelLoadCommand, ModelSpec,
    ManagedModelBackendId, ModelStatus, UnifiedModel, UnifiedModelKind, UnifiedModelStatus,
    UpdateModelCommand, normalize_model_capabilities,
};
use crate::error::AppCoreError;
use crate::infra::db::{ModelStore, UnifiedModelRecord};
use crate::infra::model_packs;
use crate::infra::rpc::{self, pb};
use crate::model_auto_unload::build_model_load_request;

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;
type CloudProviderConfig = slab_types::settings::CloudProviderConfig;

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

#[derive(Debug, Clone)]
struct ResolvedModelLoadTarget {
    backend_id: RuntimeBackendId,
    model_path: String,
    model_id: Option<String>,
    pack_load_defaults: Option<slab_model_pack::ModelPackLoadDefaults>,
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
        let command = model_packs::build_model_command_from_pack_bytes(&pack_path, bytes)?;
        let pack_existed = pack_path.exists();

        let model = self.build_model_definition(command).await?;
        model_packs::write_imported_model_pack(self.model_config_dir(), &model, bytes)?;

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

    pub async fn get_model(&self, id: &str) -> Result<UnifiedModel, AppCoreError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| AppCoreError::NotFound(format!("model {id} not found")))?;

        record.try_into().map_err(|e: String| AppCoreError::Internal(e))
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

        self.model_state.store().delete_model(id).await?;
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
            let command = match model_packs::build_model_command_from_pack(&path) {
                Ok(command) => command,
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "skipping invalid model pack file");
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

        let repo_id = model.spec.repo_id.clone().ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model {model_id} spec is missing repo_id required for download"
            ))
        })?;
        let filename = model.spec.filename.clone().ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model {model_id} spec is missing filename required for download"
            ))
        })?;

        let input_data = serde_json::json!({
            "model_id": model.id,
            "backend_id": canonical_backend_id,
            "repo_id": repo_id,
            "filename": filename,
            "model_cache_dir": configured_model_cache_dir,
        })
        .to_string();

        let store = Arc::clone(self.model_state.store());
        let model_config_dir = self.model_config_dir().to_path_buf();
        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending(
                    "model_download",
                    Some(model_id.clone()),
                    Some(input_data.clone()),
                ),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();
                    if let Err(error) = operation.mark_running().await {
                        warn!(task_id = %operation_id, error = %error, "failed to mark model download running");
                        return;
                    }

                    let input: serde_json::Value = match serde_json::from_str(&input_data) {
                        Ok(value) => value,
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "invalid stored input_data for model_download task");
                            let message = format!("invalid stored input_data: {error}");
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download parse error");
                            }
                            return;
                        }
                    };

                    let model_id = input["model_id"].as_str().unwrap_or("").to_owned();
                    let repo_id = input["repo_id"].as_str().unwrap_or("").to_owned();
                    let filename = input["filename"].as_str().unwrap_or("").to_owned();
                    let model_cache_dir = input["model_cache_dir"]
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned);

                    if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
                        warn!(task_id = %operation_id, "model_download task is missing model_id, repo_id, or filename");
                        if let Err(db_error) = operation
                            .mark_failed("missing model_id, repo_id, or filename in stored input_data")
                            .await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist model download validation error");
                        }
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
                        let path = api
                            .model(repo_id)
                            .get(&filename)
                            .map_err(|error| format!("hf-hub download failed: {error}"))?;
                        Ok::<String, String>(path.to_string_lossy().into_owned())
                    })
                    .await;

                    match result {
                        Ok(Ok(local_path)) => {
                            if let Err(error) = store
                                .update_model_local_path(&model_id, &local_path, "ready")
                                .await
                            {
                                warn!(task_id = %operation_id, error = %error, "failed to persist downloaded model path");
                                let message =
                                    format!("downloaded file but failed to persist path: {error}");
                                if let Err(db_error) = operation.mark_failed(&message).await {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist post-download failure");
                                }
                                return;
                            }

                            let updated_record = match store.get_model(&model_id).await {
                                Ok(Some(record)) => record,
                                Ok(None) => {
                                    let message = format!(
                                        "downloaded file but model {model_id} no longer exists after update"
                                    );
                                    if let Err(db_error) = operation.mark_failed(&message).await {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to persist missing model after download");
                                    }
                                    return;
                                }
                                Err(error) => {
                                    let message = format!(
                                        "downloaded file but failed to reload model {model_id}: {error}"
                                    );
                                    if let Err(db_error) = operation.mark_failed(&message).await {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to persist model reload error after download");
                                    }
                                    return;
                                }
                            };

                            if let Err(error) =
                                sync_model_pack_record(&model_config_dir, updated_record)
                            {
                                warn!(task_id = %operation_id, error = %error, "failed to sync downloaded model pack");
                                let message = format!(
                                    "downloaded file but failed to sync model pack: {error}"
                                );
                                if let Err(db_error) = operation.mark_failed(&message).await {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist config sync failure after download");
                                }
                                return;
                            }

                            let result_json =
                                serde_json::json!({ "local_path": local_path }).to_string();
                            if let Err(db_error) = operation.mark_succeeded(&result_json).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download success");
                            }
                            info!(task_id = %operation_id, local_path = %local_path, "model download succeeded");
                        }
                        Ok(Err(error)) => {
                            warn!(task_id = %operation_id, error = %error, "model download failed");
                            if let Err(db_error) = operation.mark_failed(&error).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download failure");
                            }
                        }
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "model download task panicked");
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download panic");
                            }
                        }
                    }
                },
            )
            .await?;

        info!(
            task_id = %operation_id,
            backend_id = %backend_id,
            model_id = %model_id,
            "model download task accepted"
        );

        Ok(AcceptedOperation { operation_id })
    }

    fn model_config_dir(&self) -> &std::path::Path {
        self.model_state.config().model_config_dir.as_path()
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
    let records = state.store().list_models().await?;
    let requested_capability = query.capability;
    let models = records
        .into_iter()
        .filter_map(|record| {
            record
                .try_into()
                .map_err(|error: String| {
                    warn!(error = %error, "failed to deserialize model record; skipping");
                })
                .ok()
        })
        .filter(|model: &UnifiedModel| {
            requested_capability
                .is_none_or(|capability| model.capabilities.contains(&capability))
        })
        .collect();
    Ok(models)
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
) -> Result<(), AppCoreError> {
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    model_packs::write_persisted_model_pack(config_dir, &model)?;
    Ok(())
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
    let (context_length, context_source) = if let Some(context_length) =
        resolved_target.pack_load_defaults.as_ref().and_then(|defaults| defaults.context_length)
    {
        (context_length, "model_pack")
    } else {
        resolve_llama_context_length(&state, resolved_target.backend_id).await?
    };
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
    state.auto_unload().notify_model_loaded(resolved_target.backend_id, load_spec).await;

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
        RuntimeBackendId::GgmlLlama => Ok(RuntimeBackendLoadSpec::GgmlLlama(
            GgmlLlamaLoadConfig {
                model_path,
                num_workers: usize::try_from(num_workers).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to convert num_workers into usize for ggml.llama: {error}"
                    ))
                })?,
                context_length: (context_length > 0).then_some(context_length),
                chat_template,
            },
        )),
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
                clip_device: (!diffusion.clip_device.is_empty())
                    .then_some(diffusion.clip_device),
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
            let pack_target =
                model_packs::build_model_pack_load_target(std::path::Path::new(&model_path))?;
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
