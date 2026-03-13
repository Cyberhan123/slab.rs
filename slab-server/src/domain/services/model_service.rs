use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::Utc;
use hf_hub::api::sync::{Api, ApiBuilder};
use slab_core::api::Backend;
use tonic::transport::Channel;
use tracing::{info, warn};

use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, AvailableModelsQuery, AvailableModelsView, CreateModelCommand,
    DeletedModelView, DownloadModelCommand, ListModelsFilter, ModelCatalogItemView,
    ModelCatalogStatus, ModelLoadCommand, ModelStatus, UpdateModelCommand,
};
use crate::error::ServerError;
use crate::infra::db::{ConfigStore, ModelCatalogRecord, ModelStore, TaskRecord, TaskStore};
use crate::infra::rpc::{self, pb};
use crate::model_auto_unload::LoadedModelSpec;

const MODEL_CACHE_DIR_CONFIG_KEY: &str = "model_cache_dir";
const LLAMA_NUM_WORKERS_CONFIG_KEY: &str = "llama_num_workers";
const WHISPER_NUM_WORKERS_CONFIG_KEY: &str = "whisper_num_workers";
const DIFFUSION_NUM_WORKERS_CONFIG_KEY: &str = "diffusion_num_workers";
const LLAMA_CONTEXT_LENGTH_CONFIG_KEY: &str = "llama_context_length";
const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;
const WHISPER_BACKEND_ID: &str = "ggml.whisper";

const DIFFUSION_MODEL_PATH_CONFIG_KEY: &str = "diffusion_model_path";
const DIFFUSION_VAE_PATH_CONFIG_KEY: &str = "diffusion_vae_path";
const DIFFUSION_TAESD_PATH_CONFIG_KEY: &str = "diffusion_taesd_path";
const DIFFUSION_LORA_MODEL_DIR_CONFIG_KEY: &str = "diffusion_lora_model_dir";
const DIFFUSION_CLIP_L_PATH_CONFIG_KEY: &str = "diffusion_clip_l_path";
const DIFFUSION_CLIP_G_PATH_CONFIG_KEY: &str = "diffusion_clip_g_path";
const DIFFUSION_T5XXL_PATH_CONFIG_KEY: &str = "diffusion_t5xxl_path";
const DIFFUSION_FLASH_ATTN_CONFIG_KEY: &str = "diffusion_flash_attn";
const DIFFUSION_KEEP_VAE_ON_CPU_CONFIG_KEY: &str = "diffusion_keep_vae_on_cpu";
const DIFFUSION_KEEP_CLIP_ON_CPU_CONFIG_KEY: &str = "diffusion_keep_clip_on_cpu";
const DIFFUSION_OFFLOAD_PARAMS_CONFIG_KEY: &str = "diffusion_offload_params_to_cpu";

#[derive(Clone)]
pub struct ModelService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl ModelService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self {
            model_state,
            worker_state,
        }
    }

    pub async fn create_model(
        &self,
        req: CreateModelCommand,
    ) -> Result<ModelCatalogItemView, ServerError> {
        let backend_ids = normalize_backend_ids(&req.backend_ids)?;

        let now = Utc::now();
        let record = ModelCatalogRecord {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: req.display_name.trim().to_owned(),
            repo_id: req.repo_id.trim().to_owned(),
            filename: req.filename.trim().to_owned(),
            backend_ids,
            local_path: None,
            last_download_task_id: None,
            last_downloaded_at: None,
            created_at: now,
            updated_at: now,
        };

        self.model_state
            .store()
            .insert_model(record.clone())
            .await?;
        Ok(to_model_catalog_item_view(record, None))
    }

    pub async fn update_model(
        &self,
        id: &str,
        req: UpdateModelCommand,
    ) -> Result<ModelCatalogItemView, ServerError> {
        let existing = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {id} not found")))?;
        let display_name = req
            .display_name
            .unwrap_or(existing.display_name)
            .trim()
            .to_owned();
        let repo_id = req.repo_id.unwrap_or(existing.repo_id).trim().to_owned();
        let filename = req.filename.unwrap_or(existing.filename).trim().to_owned();
        let backend_ids = if let Some(ids) = req.backend_ids {
            normalize_backend_ids(&ids)?
        } else {
            existing.backend_ids
        };

        self.model_state
            .store()
            .update_model_metadata(id, &display_name, &repo_id, &filename, &backend_ids)
            .await?;

        let updated = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {id} not found after update")))?;
        let pending_task = latest_pending_download_task_for_model(&self.model_state, id).await?;

        Ok(to_model_catalog_item_view(updated, pending_task.as_ref()))
    }

    pub async fn delete_model(&self, id: &str) -> Result<DeletedModelView, ServerError> {
        let exists = self.model_state.store().get_model(id).await?;
        if exists.is_none() {
            return Err(ServerError::NotFound(format!("model {id} not found")));
        }

        self.model_state.store().delete_model(id).await?;
        Ok(DeletedModelView {
            id: id.to_owned(),
            status: "deleted".to_owned(),
        })
    }

    pub async fn list_models(
        &self,
        query: ListModelsFilter,
    ) -> Result<Vec<ModelCatalogItemView>, ServerError> {
        let models = self.model_state.store().list_models().await?;
        let download_tasks = self
            .model_state
            .store()
            .list_tasks(Some("model_download"))
            .await?;
        let pending_by_model = pending_download_map(download_tasks);

        let mut items = Vec::with_capacity(models.len());
        for model in models {
            let pending_task = pending_by_model.get(&model.id);
            let item = to_model_catalog_item_view(model, pending_task);

            let include = match query.status {
                ModelCatalogStatus::All => true,
                _ => query.status == item.status,
            };
            if include {
                items.push(item);
            }
        }

        Ok(items)
    }

    pub async fn load_model(&self, command: ModelLoadCommand) -> Result<ModelStatus, ServerError> {
        self.load_model_command("load_model", "loading model", command)
            .await
    }

    pub async fn unload_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        let backend_id = command.backend_id;
        info!(backend = %backend_id, "unloading model");

        let (canonical_backend, channel) = resolve_backend_channel(&self.model_state, &backend_id)?;
        let response =
            rpc::client::unload_model(channel, &canonical_backend, pb::ModelUnloadRequest {})
                .await
                .map_err(|error| {
                    ServerError::Internal(format!("grpc unload_model failed: {error}"))
                })?;
        self.model_state
            .auto_unload()
            .notify_model_unloaded(&canonical_backend)
            .await;

        Ok(ModelStatus {
            backend: response.backend,
            status: response.status,
        })
    }

    pub async fn list_available_models(
        &self,
        query: AvailableModelsQuery,
    ) -> Result<AvailableModelsView, ServerError> {
        let repo_id = query.repo_id.clone();
        let files: Vec<String> = tokio::task::spawn_blocking(move || {
            let api = Api::new().map_err(|error| format!("hf-hub init failed: {error}"))?;
            let repo = api.model(repo_id);
            let info = repo
                .info()
                .map_err(|error| format!("hf-hub info failed: {error}"))?;
            let names = info
                .siblings
                .into_iter()
                .map(|item| item.rfilename)
                .collect();
            Ok::<Vec<String>, String>(names)
        })
        .await
        .map_err(|error| ServerError::Internal(format!("spawn_blocking panicked: {error}")))?
        .map_err(ServerError::Internal)?;

        Ok(AvailableModelsView {
            repo_id: query.repo_id,
            files,
        })
    }

    pub async fn switch_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        self.load_model_command("switch_model", "switching model", command)
            .await
    }

    pub async fn download_model(
        &self,
        req: DownloadModelCommand,
    ) -> Result<AcceptedOperation, ServerError> {
        let model_id = req.model_id.trim();
        let backend_id = req.backend_id.trim();

        let configured_model_cache_dir = self
            .model_state
            .store()
            .get_config_value(MODEL_CACHE_DIR_CONFIG_KEY)
            .await?
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if let Some(dir) = &configured_model_cache_dir {
            validate_path("model_cache_dir", dir)?;
        }

        let backend = Backend::from_str(backend_id)
            .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {backend_id}")))?;
        let canonical_backend_id = backend.to_string();

        let model = self
            .model_state
            .store()
            .get_model(model_id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {model_id} not found")))?;

        if !model
            .backend_ids
            .iter()
            .any(|value| value == &canonical_backend_id)
        {
            return Err(ServerError::BadRequest(format!(
                "backend_id '{canonical_backend_id}' is not configured for model {model_id}"
            )));
        }

        let input_data = serde_json::json!({
            "model_id": model.id,
            "backend_id": canonical_backend_id,
            "repo_id": model.repo_id,
            "filename": model.filename,
            "model_cache_dir": configured_model_cache_dir,
        })
        .to_string();

        let store = Arc::clone(self.model_state.store());
        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending(
                    "model_download",
                    Some(model_id.to_owned()),
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
                                .mark_model_downloaded(
                                    &model_id,
                                    &local_path,
                                    &operation_id,
                                    chrono::Utc::now(),
                                )
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

    async fn load_model_command(
        &self,
        action: &'static str,
        log_message: &'static str,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        load_model_with_state(self.model_state.clone(), action, log_message, command).await
    }
}

fn validate_path(label: &str, path: &str) -> Result<(), ServerError> {
    if path.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "{label} must not be empty"
        )));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(ServerError::BadRequest(format!(
            "{label} must be an absolute path (got: {path})"
        )));
    }
    let has_traversal = std::path::Path::new(path)
        .components()
        .any(|component| component == std::path::Component::ParentDir);
    if has_traversal {
        return Err(ServerError::BadRequest(format!(
            "{label} must not contain '..' components"
        )));
    }
    Ok(())
}

fn validate_existing_model_file(path: &str) -> Result<(), ServerError> {
    let model_path = std::path::Path::new(path);
    if !model_path.exists() {
        return Err(ServerError::BadRequest(format!(
            "model_path does not exist: {path}"
        )));
    }
    if !model_path.is_file() {
        return Err(ServerError::BadRequest(format!(
            "model_path is not a file: {path}"
        )));
    }
    Ok(())
}

fn normalize_backend_ids(raw: &[String]) -> Result<Vec<String>, ServerError> {
    if raw.is_empty() {
        return Err(ServerError::BadRequest(
            "backend_ids must include at least one backend".into(),
        ));
    }

    let mut out = Vec::with_capacity(raw.len());
    for backend_id in raw {
        let trimmed = backend_id.trim();
        if trimmed.is_empty() {
            return Err(ServerError::BadRequest(
                "backend_ids must not contain empty values".into(),
            ));
        }
        let backend = Backend::from_str(trimmed)
            .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {trimmed}")))?;
        out.push(backend.to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn to_model_catalog_item_view(
    model: ModelCatalogRecord,
    pending_task: Option<&TaskRecord>,
) -> ModelCatalogItemView {
    let computed_status = if model.local_path.is_some() {
        ModelCatalogStatus::Downloaded
    } else if pending_task.is_some() {
        ModelCatalogStatus::Pending
    } else {
        ModelCatalogStatus::NotDownloaded
    };

    let is_vad_model = detect_whisper_vad_model(
        &model.backend_ids,
        &model.display_name,
        &model.repo_id,
        &model.filename,
    );

    ModelCatalogItemView {
        id: model.id,
        display_name: model.display_name,
        repo_id: model.repo_id,
        filename: model.filename,
        backend_ids: model.backend_ids,
        is_vad_model,
        status: computed_status,
        local_path: model.local_path,
        last_downloaded_at: model.last_downloaded_at.map(|value| value.to_rfc3339()),
        pending_task_id: pending_task.map(|task| task.id.clone()),
        pending_task_status: pending_task.map(|task| task.status.clone()),
    }
}

fn pending_download_map(tasks: Vec<TaskRecord>) -> HashMap<String, TaskRecord> {
    let mut pending_by_model = HashMap::new();
    for task in tasks {
        if !matches!(task.status.as_str(), "pending" | "running") {
            continue;
        }
        let Some(model_id) = task.model_id.clone() else {
            continue;
        };
        let should_replace = pending_by_model
            .get(&model_id)
            .map(|current: &TaskRecord| task.updated_at > current.updated_at)
            .unwrap_or(true);
        if should_replace {
            pending_by_model.insert(model_id, task);
        }
    }
    pending_by_model
}

fn resolve_backend_channel(
    state: &ModelState,
    backend_id: &str,
) -> Result<(String, Channel), ServerError> {
    let backend = Backend::from_str(backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let canonical_backend = backend.to_string();
    let channel = state
        .grpc()
        .backend_channel(&canonical_backend)
        .ok_or_else(|| {
            ServerError::BackendNotReady(format!(
                "{canonical_backend} gRPC endpoint is not configured"
            ))
        })?;
    Ok((canonical_backend, channel))
}

fn workers_config_key_for_backend(backend_id: &str) -> Option<&'static str> {
    match backend_id {
        "ggml.llama" => Some(LLAMA_NUM_WORKERS_CONFIG_KEY),
        "ggml.whisper" => Some(WHISPER_NUM_WORKERS_CONFIG_KEY),
        "ggml.diffusion" => Some(DIFFUSION_NUM_WORKERS_CONFIG_KEY),
        _ => None,
    }
}

fn parse_positive_u32(raw: &str, key: &str) -> Result<u32, ServerError> {
    let trimmed = raw.trim();
    let parsed = trimmed.parse::<u32>().map_err(|_| {
        ServerError::BadRequest(format!("config key '{key}' must be a positive integer"))
    })?;

    if parsed == 0 {
        return Err(ServerError::BadRequest(format!(
            "config key '{key}' must be at least 1"
        )));
    }

    Ok(parsed)
}

fn parse_num_workers(raw: &str, key: &str) -> Result<u32, ServerError> {
    parse_positive_u32(raw, key)
}

fn detect_whisper_vad_model(
    backend_ids: &[String],
    display_name: &str,
    repo_id: &str,
    filename: &str,
) -> bool {
    if !backend_ids.iter().any(|value| value == WHISPER_BACKEND_ID) {
        return false;
    }

    let haystack = format!(
        "{} {} {}",
        display_name.to_ascii_lowercase(),
        repo_id.to_ascii_lowercase(),
        filename.to_ascii_lowercase()
    );

    [
        " vad", "vad ", "-vad", "_vad", "vad-", "vad_", "silero", "fsmn-vad",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
        || haystack.ends_with("vad")
}

async fn resolve_model_workers(
    state: &ModelState,
    canonical_backend: &str,
    requested_workers: Option<u32>,
) -> Result<(u32, &'static str), ServerError> {
    if let Some(workers) = requested_workers {
        if workers == 0 {
            return Err(ServerError::BadRequest(
                "num_workers must be at least 1".into(),
            ));
        }
        return Ok((workers, "request"));
    }

    let Some(config_key) = workers_config_key_for_backend(canonical_backend) else {
        return Ok((DEFAULT_MODEL_NUM_WORKERS, "default"));
    };

    let configured = state.store().get_config_value(config_key).await?;
    let Some(raw) = configured else {
        return Ok((DEFAULT_MODEL_NUM_WORKERS, "default"));
    };

    if raw.trim().is_empty() {
        return Ok((DEFAULT_MODEL_NUM_WORKERS, "default"));
    }

    let workers = parse_num_workers(&raw, config_key)?;
    Ok((workers, "config"))
}

async fn resolve_llama_context_length(
    state: &ModelState,
    canonical_backend: &str,
) -> Result<(u32, &'static str), ServerError> {
    if canonical_backend != "ggml.llama" {
        return Ok((0, "not_applicable"));
    }

    let configured = state
        .store()
        .get_config_value(LLAMA_CONTEXT_LENGTH_CONFIG_KEY)
        .await?;
    let Some(raw) = configured else {
        return Ok((0, "default"));
    };
    if raw.trim().is_empty() {
        return Ok((0, "default"));
    }

    let context_length = parse_positive_u32(&raw, LLAMA_CONTEXT_LENGTH_CONFIG_KEY)?;
    Ok((context_length, "config"))
}

struct DiffusionContextParams {
    diffusion_model_path: String,
    vae_path: String,
    taesd_path: String,
    lora_model_dir: String,
    clip_l_path: String,
    clip_g_path: String,
    t5xxl_path: String,
    flash_attn: bool,
    keep_vae_on_cpu: bool,
    keep_clip_on_cpu: bool,
    offload_params_to_cpu: bool,
}

impl Default for DiffusionContextParams {
    fn default() -> Self {
        Self {
            diffusion_model_path: String::new(),
            vae_path: String::new(),
            taesd_path: String::new(),
            lora_model_dir: String::new(),
            clip_l_path: String::new(),
            clip_g_path: String::new(),
            t5xxl_path: String::new(),
            flash_attn: false,
            keep_vae_on_cpu: false,
            keep_clip_on_cpu: false,
            offload_params_to_cpu: false,
        }
    }
}

async fn resolve_diffusion_context_params(
    state: &ModelState,
    canonical_backend: &str,
) -> Result<Option<DiffusionContextParams>, ServerError> {
    if canonical_backend != "ggml.diffusion" {
        return Ok(None);
    }

    async fn get_str(state: &ModelState, key: &str) -> Result<String, ServerError> {
        Ok(state
            .store()
            .get_config_value(key)
            .await?
            .unwrap_or_default())
    }

    async fn get_bool(state: &ModelState, key: &str) -> Result<bool, ServerError> {
        let raw = state
            .store()
            .get_config_value(key)
            .await?
            .unwrap_or_default();
        Ok(["1", "true", "yes"].contains(&raw.trim().to_lowercase().as_str()))
    }

    Ok(Some(DiffusionContextParams {
        diffusion_model_path: get_str(state, DIFFUSION_MODEL_PATH_CONFIG_KEY).await?,
        vae_path: get_str(state, DIFFUSION_VAE_PATH_CONFIG_KEY).await?,
        taesd_path: get_str(state, DIFFUSION_TAESD_PATH_CONFIG_KEY).await?,
        lora_model_dir: get_str(state, DIFFUSION_LORA_MODEL_DIR_CONFIG_KEY).await?,
        clip_l_path: get_str(state, DIFFUSION_CLIP_L_PATH_CONFIG_KEY).await?,
        clip_g_path: get_str(state, DIFFUSION_CLIP_G_PATH_CONFIG_KEY).await?,
        t5xxl_path: get_str(state, DIFFUSION_T5XXL_PATH_CONFIG_KEY).await?,
        flash_attn: get_bool(state, DIFFUSION_FLASH_ATTN_CONFIG_KEY).await?,
        keep_vae_on_cpu: get_bool(state, DIFFUSION_KEEP_VAE_ON_CPU_CONFIG_KEY).await?,
        keep_clip_on_cpu: get_bool(state, DIFFUSION_KEEP_CLIP_ON_CPU_CONFIG_KEY).await?,
        offload_params_to_cpu: get_bool(state, DIFFUSION_OFFLOAD_PARAMS_CONFIG_KEY).await?,
    }))
}

fn grpc_status_message(status: &tonic::Status) -> String {
    let message = status.message().trim();
    if !message.is_empty() {
        return message.to_owned();
    }
    status.to_string()
}

fn map_grpc_model_error(action: &str, err: anyhow::Error) -> ServerError {
    let grpc_status = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<tonic::Status>());

    if let Some(status) = grpc_status {
        let detail = grpc_status_message(status);
        return match status.code() {
            tonic::Code::InvalidArgument
            | tonic::Code::FailedPrecondition
            | tonic::Code::ResourceExhausted => ServerError::BadRequest(detail),
            tonic::Code::NotFound => ServerError::NotFound(detail),
            tonic::Code::Unavailable => ServerError::BackendNotReady(detail),
            _ => ServerError::Internal(format!("grpc {action} failed: {err:#}")),
        };
    }

    ServerError::Internal(format!("grpc {action} failed: {err:#}"))
}

async fn latest_pending_download_task_for_model(
    state: &ModelState,
    model_id: &str,
) -> Result<Option<TaskRecord>, ServerError> {
    let tasks = state.store().list_tasks(Some("model_download")).await?;
    let mut pending_by_model = pending_download_map(tasks);
    Ok(pending_by_model.remove(model_id))
}

async fn load_model_with_state(
    state: ModelState,
    action: &'static str,
    log_message: &'static str,
    command: ModelLoadCommand,
) -> Result<ModelStatus, ServerError> {
    let backend_id = &command.backend_id;

    validate_path("model_path", &command.model_path)?;
    validate_existing_model_file(&command.model_path)?;

    let (canonical_backend, channel) = resolve_backend_channel(&state, backend_id)?;
    let (num_workers, worker_source) =
        resolve_model_workers(&state, &canonical_backend, command.num_workers).await?;
    let (context_length, context_source) =
        resolve_llama_context_length(&state, &canonical_backend).await?;

    info!(
        backend = %backend_id,
        model_path = %command.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "{log_message}"
    );

    let diffusion_ctx = resolve_diffusion_context_params(&state, &canonical_backend)
        .await?
        .unwrap_or_default();

    let grpc_req = pb::ModelLoadRequest {
        model_path: command.model_path.clone(),
        num_workers,
        context_length,
        diffusion_model_path: diffusion_ctx.diffusion_model_path,
        vae_path: diffusion_ctx.vae_path,
        taesd_path: diffusion_ctx.taesd_path,
        lora_model_dir: diffusion_ctx.lora_model_dir,
        clip_l_path: diffusion_ctx.clip_l_path,
        clip_g_path: diffusion_ctx.clip_g_path,
        t5xxl_path: diffusion_ctx.t5xxl_path,
        flash_attn: diffusion_ctx.flash_attn,
        keep_vae_on_cpu: diffusion_ctx.keep_vae_on_cpu,
        keep_clip_on_cpu: diffusion_ctx.keep_clip_on_cpu,
        offload_params_to_cpu: diffusion_ctx.offload_params_to_cpu,
    };
    let response = rpc::client::load_model(channel, &canonical_backend, grpc_req)
        .await
        .map_err(|error| map_grpc_model_error(action, error))?;
    state
        .auto_unload()
        .notify_model_loaded(
            &canonical_backend,
            LoadedModelSpec {
                model_path: command.model_path,
                num_workers,
                context_length,
            },
        )
        .await;

    Ok(ModelStatus {
        backend: response.backend,
        status: response.status,
    })
}
