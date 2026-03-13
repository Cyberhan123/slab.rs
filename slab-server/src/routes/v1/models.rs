//! Model-management routes.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::Utc;
use tonic::transport::Channel;
use tracing::{info, warn};
use utoipa::OpenApi;

use crate::contexts::model::application::load_model_use_case::{LoadModelUseCase, ModelLoadPort};
use crate::entities::{ConfigStore, ModelCatalogRecord, ModelStore, TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::grpc;
use crate::model_auto_unload::LoadedModelSpec;
use crate::schemas::v1::models::{
    CreateModelRequest, DownloadModelRequest, ListAvailableQuery, ListModelsQuery,
    LoadModelRequest, ModelCatalogItemResponse, ModelListStatus, ModelStatusResponse,
    SwitchModelRequest, UpdateModelRequest,
};
use crate::state::ModelContext;

use super::V1State;
use hf_hub::api::sync::{Api, ApiBuilder};
use slab_core::api::Backend;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_models,
        create_model,
        update_model,
        delete_model,
        load_model,
        unload_model,
        list_available_models,
        switch_model,
        download_model
    ),
    components(schemas(
        CreateModelRequest,
        UpdateModelRequest,
        LoadModelRequest,
        ModelStatusResponse,
        SwitchModelRequest,
        DownloadModelRequest,
        ListAvailableQuery,
        ListModelsQuery,
        ModelCatalogItemResponse
    ))
)]
pub struct ModelsApi;

const MODEL_CACHE_DIR_CONFIG_KEY: &str = "model_cache_dir";
const LLAMA_NUM_WORKERS_CONFIG_KEY: &str = "llama_num_workers";
const WHISPER_NUM_WORKERS_CONFIG_KEY: &str = "whisper_num_workers";
const DIFFUSION_NUM_WORKERS_CONFIG_KEY: &str = "diffusion_num_workers";
const LLAMA_CONTEXT_LENGTH_CONFIG_KEY: &str = "llama_context_length";
const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;
const WHISPER_BACKEND_ID: &str = "ggml.whisper";

// Diffusion-specific global config keys (used when loading a diffusion model).
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

/// Register model-management routes.
pub fn router() -> Router<Arc<V1State>> {
    Router::new()
        .route("/models", get(list_models).post(create_model))
        .route("/models/{id}", put(update_model).delete(delete_model))
        .route("/models/available", get(list_available_models))
        .route("/models/load", post(load_model))
        .route("/models/unload", post(unload_model))
        .route("/models/switch", post(switch_model))
        .route("/models/download", post(download_model))
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
        .any(|c| c == std::path::Component::ParentDir);
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

fn validate_catalog_fields(
    display_name: &str,
    repo_id: &str,
    filename: &str,
) -> Result<(), ServerError> {
    if display_name.trim().is_empty() {
        return Err(ServerError::BadRequest(
            "display_name must not be empty".into(),
        ));
    }
    if repo_id.trim().is_empty() {
        return Err(ServerError::BadRequest("repo_id must not be empty".into()));
    }
    if filename.trim().is_empty() {
        return Err(ServerError::BadRequest("filename must not be empty".into()));
    }
    Ok(())
}

fn to_model_catalog_item_response(
    model: ModelCatalogRecord,
    pending_task: Option<&TaskRecord>,
) -> ModelCatalogItemResponse {
    let computed_status = if model.local_path.is_some() {
        ModelListStatus::Downloaded
    } else if pending_task.is_some() {
        ModelListStatus::Pending
    } else {
        ModelListStatus::NotDownloaded
    };

    let is_vad_model = detect_whisper_vad_model(
        &model.backend_ids,
        &model.display_name,
        &model.repo_id,
        &model.filename,
    );

    ModelCatalogItemResponse {
        id: model.id,
        display_name: model.display_name,
        repo_id: model.repo_id,
        filename: model.filename,
        backend_ids: model.backend_ids,
        is_vad_model,
        status: computed_status,
        local_path: model.local_path,
        last_downloaded_at: model.last_downloaded_at.map(|v| v.to_rfc3339()),
        pending_task_id: pending_task.map(|t| t.id.clone()),
        pending_task_status: pending_task.map(|t| t.status.clone()),
    }
}

fn resolve_backend_channel(
    context: &ModelContext,
    backend_id: &str,
) -> Result<(String, Channel), ServerError> {
    let backend = Backend::from_str(backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let canonical_backend = backend.to_string();
    let channel = context
        .grpc
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
    if !backend_ids.iter().any(|v| v == WHISPER_BACKEND_ID) {
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
    context: &ModelContext,
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

    let configured = context.store.get_config_value(config_key).await?;
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
    context: &ModelContext,
    canonical_backend: &str,
) -> Result<(u32, &'static str), ServerError> {
    if canonical_backend != "ggml.llama" {
        return Ok((0, "not_applicable"));
    }

    let configured = context
        .store
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

/// Named set of diffusion model-context parameters resolved from the admin config store.
/// This struct is returned by `resolve_diffusion_context_params` and mapped directly
/// into `grpc::pb::ModelLoadRequest` fields, making call-sites readable and extension-safe.
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

/// Resolve diffusion model context parameters from the admin config store.
///
/// Returns `None` for non-diffusion backends.  All fields default to empty
/// string / `false` when the corresponding config key is not set.
async fn resolve_diffusion_context_params(
    context: &ModelContext,
    canonical_backend: &str,
) -> Result<Option<DiffusionContextParams>, ServerError> {
    if canonical_backend != "ggml.diffusion" {
        return Ok(None);
    }

    async fn get_str(context: &ModelContext, key: &str) -> Result<String, ServerError> {
        Ok(context
            .store
            .get_config_value(key)
            .await?
            .unwrap_or_default())
    }

    async fn get_bool(context: &ModelContext, key: &str) -> Result<bool, ServerError> {
        let raw = context
            .store
            .get_config_value(key)
            .await?
            .unwrap_or_default();
        Ok(["1", "true", "yes"].contains(&raw.trim().to_lowercase().as_str()))
    }

    Ok(Some(DiffusionContextParams {
        diffusion_model_path: get_str(context, DIFFUSION_MODEL_PATH_CONFIG_KEY).await?,
        vae_path: get_str(context, DIFFUSION_VAE_PATH_CONFIG_KEY).await?,
        taesd_path: get_str(context, DIFFUSION_TAESD_PATH_CONFIG_KEY).await?,
        lora_model_dir: get_str(context, DIFFUSION_LORA_MODEL_DIR_CONFIG_KEY).await?,
        clip_l_path: get_str(context, DIFFUSION_CLIP_L_PATH_CONFIG_KEY).await?,
        clip_g_path: get_str(context, DIFFUSION_CLIP_G_PATH_CONFIG_KEY).await?,
        t5xxl_path: get_str(context, DIFFUSION_T5XXL_PATH_CONFIG_KEY).await?,
        flash_attn: get_bool(context, DIFFUSION_FLASH_ATTN_CONFIG_KEY).await?,
        keep_vae_on_cpu: get_bool(context, DIFFUSION_KEEP_VAE_ON_CPU_CONFIG_KEY).await?,
        keep_clip_on_cpu: get_bool(context, DIFFUSION_KEEP_CLIP_ON_CPU_CONFIG_KEY).await?,
        offload_params_to_cpu: get_bool(context, DIFFUSION_OFFLOAD_PARAMS_CONFIG_KEY).await?,
    }))
}

fn grpc_status_message(status: &tonic::Status) -> String {
    let msg = status.message().trim();
    if !msg.is_empty() {
        return msg.to_owned();
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
    context: &ModelContext,
    model_id: &str,
) -> Result<Option<TaskRecord>, ServerError> {
    let tasks = context.store.list_tasks(Some("model_download")).await?;
    Ok(tasks
        .into_iter()
        .filter(|task| {
            task.model_id.as_deref() == Some(model_id)
                && matches!(task.status.as_str(), "pending" | "running")
        })
        .max_by_key(|task| task.updated_at))
}

#[utoipa::path(
    post,
    path = "/v1/models",
    tag = "models",
    request_body = CreateModelRequest,
    responses(
        (status = 200, description = "Model catalog entry created", body = ModelCatalogItemResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn create_model(
    State(context): State<Arc<ModelContext>>,
    Json(req): Json<CreateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    validate_catalog_fields(&req.display_name, &req.repo_id, &req.filename)?;
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

    context.store.insert_model(record.clone()).await?;
    Ok(Json(to_model_catalog_item_response(record, None)))
}

#[utoipa::path(
    put,
    path = "/v1/models/{id}",
    tag = "models",
    request_body = UpdateModelRequest,
    params(
        ("id" = String, Path, description = "Model catalog entry ID")
    ),
    responses(
        (status = 200, description = "Model catalog entry updated", body = ModelCatalogItemResponse),
        (status = 400, description = "Bad request"),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn update_model(
    State(context): State<Arc<ModelContext>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<Json<ModelCatalogItemResponse>, ServerError> {
    let existing = context
        .store
        .get_model(&id)
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

    validate_catalog_fields(&display_name, &repo_id, &filename)?;

    context
        .store
        .update_model_metadata(&id, &display_name, &repo_id, &filename, &backend_ids)
        .await?;

    let updated = context
        .store
        .get_model(&id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("model {id} not found after update")))?;
    let pending_task = latest_pending_download_task_for_model(&context, &id).await?;

    Ok(Json(to_model_catalog_item_response(
        updated,
        pending_task.as_ref(),
    )))
}

#[utoipa::path(
    delete,
    path = "/v1/models/{id}",
    tag = "models",
    params(
        ("id" = String, Path, description = "Model catalog entry ID")
    ),
    responses(
        (status = 200, description = "Model catalog entry deleted", body = serde_json::Value),
        (status = 404, description = "Model not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn delete_model(
    State(context): State<Arc<ModelContext>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let exists = context.store.get_model(&id).await?;
    if exists.is_none() {
        return Err(ServerError::NotFound(format!("model {id} not found")));
    }

    context.store.delete_model(&id).await?;
    Ok(Json(serde_json::json!({ "id": id, "status": "deleted" })))
}

#[utoipa::path(
    get,
    path = "/v1/models",
    tag = "models",
    params(ListModelsQuery),
    responses(
        (status = 200, description = "List model catalog entries by download status", body = [ModelCatalogItemResponse]),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn list_models(
    State(context): State<Arc<ModelContext>>,
    axum::extract::Query(q): axum::extract::Query<ListModelsQuery>,
) -> Result<Json<Vec<ModelCatalogItemResponse>>, ServerError> {
    let models = context.store.list_models().await?;
    let download_tasks = context.store.list_tasks(Some("model_download")).await?;

    // Keep the most recent pending/running model download task per model_id.
    let mut pending_by_model: HashMap<String, TaskRecord> = HashMap::new();
    for task in download_tasks {
        if !matches!(task.status.as_str(), "pending" | "running") {
            continue;
        }
        let Some(model_id) = task.model_id.clone() else {
            continue;
        };
        let should_replace = pending_by_model
            .get(&model_id)
            .map(|current| task.updated_at > current.updated_at)
            .unwrap_or(true);
        if should_replace {
            pending_by_model.insert(model_id, task);
        }
    }

    let mut items = Vec::with_capacity(models.len());
    for model in models {
        let pending_task = pending_by_model.get(&model.id);
        let item = to_model_catalog_item_response(model, pending_task);

        let include = match q.status {
            ModelListStatus::All => true,
            _ => q.status == item.status,
        };
        if !include {
            continue;
        }
        items.push(item);
    }

    Ok(Json(items))
}

/// Load (or hot-reload) a model (`POST /v1/models/load`).
#[utoipa::path(
    post,
    path = "/v1/models/load",
    tag = "models",
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model load initiated", body = ModelStatusResponse),
        (status = 400, description = "Unknown backend or invalid paths"),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn load_model(
    State(context): State<Arc<ModelContext>>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let use_case = LoadModelUseCase::new(ModelRoutePort { context });
    let result = use_case.execute(req).await?;
    Ok(Json(result))
}

struct ModelRoutePort {
    context: Arc<ModelContext>,
}

impl ModelLoadPort for ModelRoutePort {
    fn load_model(
        &self,
        req: LoadModelRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ModelStatusResponse, ServerError>> + Send + '_>,
    > {
        Box::pin(load_model_with_state(Arc::clone(&self.context), req))
    }
}

pub(crate) async fn load_model_with_state(
    context: Arc<ModelContext>,
    req: LoadModelRequest,
) -> Result<ModelStatusResponse, ServerError> {
    let bid = &req.backend_id;

    validate_path("model_path", &req.model_path)?;
    validate_existing_model_file(&req.model_path)?;

    let (canonical_backend, channel) = resolve_backend_channel(&context, bid)?;
    let (num_workers, worker_source) =
        resolve_model_workers(&context, &canonical_backend, req.num_workers).await?;
    let (context_length, context_source) =
        resolve_llama_context_length(&context, &canonical_backend).await?;

    info!(
        backend = %bid,
        model_path = %req.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "loading model"
    );

    let diffusion_ctx = resolve_diffusion_context_params(&context, &canonical_backend)
        .await?
        .unwrap_or_default();

    let grpc_req = grpc::pb::ModelLoadRequest {
        model_path: req.model_path.clone(),
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
    let response = grpc::client::load_model(channel, &canonical_backend, grpc_req)
        .await
        .map_err(|e| map_grpc_model_error("load_model", e))?;
    context
        .model_auto_unload
        .notify_model_loaded(
            &canonical_backend,
            LoadedModelSpec {
                model_path: req.model_path,
                num_workers,
                context_length,
            },
        )
        .await;

    Ok(ModelStatusResponse {
        backend: response.backend,
        status: response.status,
    })
}

/// Unload the currently loaded model (`POST /v1/models/unload`).
#[utoipa::path(
    post,
    path = "/v1/models/unload",
    tag = "models",
    request_body = LoadModelRequest,
    responses(
        (status = 202, description = "Task accepted", body = ModelStatusResponse),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn unload_model(
    State(context): State<Arc<ModelContext>>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = &req.backend_id;

    info!(backend = %bid, "unloading model");

    let (canonical_backend, channel) = resolve_backend_channel(&context, bid)?;
    let response =
        grpc::client::unload_model(channel, &canonical_backend, grpc::pb::ModelUnloadRequest {})
            .await
            .map_err(|e| ServerError::Internal(format!("grpc unload_model failed: {e}")))?;
    context
        .model_auto_unload
        .notify_model_unloaded(&canonical_backend)
        .await;

    Ok(Json(ModelStatusResponse {
        backend: response.backend,
        status: response.status,
    }))
}

/// List the files available in a HuggingFace model repo (`GET /v1/models/available?repo_id=...`).
#[utoipa::path(
        get,
        path = "/v1/models/available",
        tag = "models",
        params(ListAvailableQuery),
        responses(
            (status = 200, description = "List of available files", body = serde_json::Value),
            (status = 400, description = "Bad request (invalid parameters)"),
            (status = 500, description = "Backend error"),
        )
    )]
pub async fn list_available_models(
    State(_context): State<Arc<ModelContext>>,
    axum::extract::Query(q): axum::extract::Query<ListAvailableQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if q.repo_id.is_empty() {
        return Err(ServerError::BadRequest(
            "repo_id query parameter must not be empty".into(),
        ));
    }

    let repo_id = q.repo_id.clone();
    let files: Vec<String> = tokio::task::spawn_blocking(move || {
        let api = Api::new().map_err(|e| format!("hf-hub init failed: {e}"))?;
        let repo = api.model(repo_id);
        let info = repo
            .info()
            .map_err(|e| format!("hf-hub info failed: {e}"))?;
        let names = info.siblings.into_iter().map(|s| s.rfilename).collect();
        Ok::<Vec<String>, String>(names)
    })
    .await
    .map_err(|e| ServerError::Internal(format!("spawn_blocking panicked: {e}")))?
    .map_err(ServerError::Internal)?;

    Ok(Json(
        serde_json::json!({ "repo_id": q.repo_id, "files": files }),
    ))
}

/// Switch the loaded model to a different weights file (`POST /v1/models/switch`).
#[utoipa::path(
    post,
    path = "/v1/models/switch",
    tag = "models",
    request_body = SwitchModelRequest,
    responses(
        (status = 200, description = "Model switched successfully", body = ModelStatusResponse),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn switch_model(
    State(context): State<Arc<ModelContext>>,
    Json(req): Json<SwitchModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = &req.backend_id;
    validate_path("model_path", &req.model_path)?;
    validate_existing_model_file(&req.model_path)?;

    let (canonical_backend, channel) = resolve_backend_channel(&context, bid)?;
    let (num_workers, worker_source) =
        resolve_model_workers(&context, &canonical_backend, req.num_workers).await?;
    let (context_length, context_source) =
        resolve_llama_context_length(&context, &canonical_backend).await?;

    info!(
        backend = %bid,
        model_path = %req.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "switching model"
    );

    let switch_diffusion_ctx = resolve_diffusion_context_params(&context, &canonical_backend)
        .await?
        .unwrap_or_default();

    let response = grpc::client::load_model(
        channel,
        &canonical_backend,
        grpc::pb::ModelLoadRequest {
            model_path: req.model_path.clone(),
            num_workers,
            context_length,
            diffusion_model_path: switch_diffusion_ctx.diffusion_model_path,
            vae_path: switch_diffusion_ctx.vae_path,
            taesd_path: switch_diffusion_ctx.taesd_path,
            lora_model_dir: switch_diffusion_ctx.lora_model_dir,
            clip_l_path: switch_diffusion_ctx.clip_l_path,
            clip_g_path: switch_diffusion_ctx.clip_g_path,
            t5xxl_path: switch_diffusion_ctx.t5xxl_path,
            flash_attn: switch_diffusion_ctx.flash_attn,
            keep_vae_on_cpu: switch_diffusion_ctx.keep_vae_on_cpu,
            keep_clip_on_cpu: switch_diffusion_ctx.keep_clip_on_cpu,
            offload_params_to_cpu: switch_diffusion_ctx.offload_params_to_cpu,
        },
    )
    .await
    .map_err(|e| map_grpc_model_error("switch_model", e))?;
    context
        .model_auto_unload
        .notify_model_loaded(
            &canonical_backend,
            LoadedModelSpec {
                model_path: req.model_path,
                num_workers,
                context_length,
            },
        )
        .await;

    Ok(Json(ModelStatusResponse {
        backend: response.backend,
        status: response.status,
    }))
}

/// Download a model file from HuggingFace (`POST /v1/models/download`).
#[utoipa::path(
    post,
    path = "/v1/models/download",
    tag = "models",
    request_body = DownloadModelRequest,
    responses(
        (status = 200, description = "Download task created", body = serde_json::Value),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 404, description = "Model catalog entry not found"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn download_model(
    State(context): State<Arc<ModelContext>>,
    Json(req): Json<DownloadModelRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let model_id = req.model_id.trim();
    if model_id.is_empty() {
        return Err(ServerError::BadRequest("model_id must not be empty".into()));
    }

    let backend_id = req.backend_id.trim();
    if backend_id.is_empty() {
        return Err(ServerError::BadRequest(
            "backend_id must not be empty".into(),
        ));
    }

    let configured_model_cache_dir = context
        .store
        .get_config_value(MODEL_CACHE_DIR_CONFIG_KEY)
        .await?
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    let effective_model_cache_dir = configured_model_cache_dir;
    if let Some(dir) = &effective_model_cache_dir {
        validate_path("model_cache_dir", dir)?;
    }

    let backend = Backend::from_str(backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {backend_id}")))?;
    let canonical_backend_id = backend.to_string();

    let model = context
        .store
        .get_model(model_id)
        .await?
        .ok_or_else(|| ServerError::NotFound(format!("model {model_id} not found")))?;

    if !model.backend_ids.iter().any(|v| v == &canonical_backend_id) {
        return Err(ServerError::BadRequest(format!(
            "backend_id '{canonical_backend_id}' is not configured for model {model_id}"
        )));
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let input_data = serde_json::json!({
        "model_id":   model.id,
        "backend_id": canonical_backend_id,
        "repo_id":    model.repo_id,
        "filename":   model.filename,
        "model_cache_dir": effective_model_cache_dir,
    })
    .to_string();

    context
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "model_download".into(),
            status: "pending".into(),
            model_id: Some(model_id.to_owned()),
            input_data: Some(input_data.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&context.store);
    let task_manager = Arc::clone(&context.task_manager);
    let tid = task_id.clone();

    let join = tokio::spawn(async move {
        store
            .update_task_status(&tid, "running", None, None)
            .await
            .ok();

        let input: serde_json::Value = match serde_json::from_str(&input_data) {
            Ok(v) => v,
            Err(e) => {
                warn!(task_id = %tid, error = %e, "invalid stored input_data for model_download task");
                store
                    .update_task_status(
                        &tid,
                        "failed",
                        None,
                        Some(&format!("invalid stored input_data: {e}")),
                    )
                    .await
                    .ok();
                task_manager.remove(&tid);
                return;
            }
        };

        let model_id = input["model_id"].as_str().unwrap_or("").to_owned();
        let repo_id = input["repo_id"].as_str().unwrap_or("").to_owned();
        let filename = input["filename"].as_str().unwrap_or("").to_owned();
        let model_cache_dir = input["model_cache_dir"]
            .as_str()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned);

        if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
            warn!(task_id = %tid, "model_download task is missing model_id, repo_id, or filename");
            store
                .update_task_status(
                    &tid,
                    "failed",
                    None,
                    Some("missing model_id, repo_id, or filename in stored input_data"),
                )
                .await
                .ok();
            task_manager.remove(&tid);
            return;
        }

        let result = tokio::task::spawn_blocking(move || {
            let api = if let Some(dir) = model_cache_dir {
                ApiBuilder::new()
                    // TODO: support with_endpoint and proxy
                    .with_cache_dir(std::path::PathBuf::from(dir))
                    .build()
                    .map_err(|e| format!("hf-hub build failed: {e}"))?
            } else {
                Api::new().map_err(|e| format!("hf-hub init failed: {e}"))?
            };
            let path = api
                .model(repo_id)
                .get(&filename)
                .map_err(|e| format!("hf-hub download failed: {e}"))?;
            Ok::<String, String>(path.to_string_lossy().into_owned())
        })
        .await;

        match result {
            Ok(Ok(local_path)) => {
                if let Err(e) = store
                    .mark_model_downloaded(&model_id, &local_path, &tid, chrono::Utc::now())
                    .await
                {
                    warn!(task_id = %tid, error = %e, "failed to persist downloaded model path");
                    store
                        .update_task_status(
                            &tid,
                            "failed",
                            None,
                            Some(&format!("downloaded file but failed to persist path: {e}")),
                        )
                        .await
                        .ok();
                    task_manager.remove(&tid);
                    return;
                }
                let result_json = serde_json::json!({ "local_path": local_path }).to_string();
                store
                    .update_task_status(&tid, "succeeded", Some(&result_json), None)
                    .await
                    .ok();
                info!(task_id = %tid, local_path = %local_path, "model download succeeded");
            }
            Ok(Err(e)) => {
                warn!(task_id = %tid, error = %e, "model download failed");
                store
                    .update_task_status(&tid, "failed", None, Some(&e))
                    .await
                    .ok();
            }
            Err(e) => {
                warn!(task_id = %tid, error = %e, "model download task panicked");
                store
                    .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
            }
        }
        task_manager.remove(&tid);
    });

    context
        .task_manager
        .insert(task_id.clone(), join.abort_handle());
    info!(
        task_id = %task_id,
        backend_id = %backend_id,
        model_id = %model_id,
        "model download task accepted"
    );
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}
