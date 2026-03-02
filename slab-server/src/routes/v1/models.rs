//! Model-management routes.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use tracing::{info, warn};
use utoipa::OpenApi;

use crate::entities::{ConfigStore, ModelStore, TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::schemas::v1::models::{
    DownloadModelRequest, ListAvailableQuery, ListModelsQuery, LoadModelRequest,
    ModelCatalogItemResponse, ModelListStatus, ModelStatusResponse, SwitchModelRequest,
};
use crate::state::AppState;
use hf_hub::api::sync::{Api, ApiBuilder};
use slab_core::api::Backend;

#[derive(OpenApi)]
#[openapi(
    paths(
        list_models,
        load_model,
        unload_model,
        list_available_models,
        switch_model,
        download_model
    ),
    components(schemas(
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

const MODEL_TARGET_DIR_CONFIG_KEY: &str = "target_dir";

/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models", get(list_models))
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
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<ListModelsQuery>,
) -> Result<Json<Vec<ModelCatalogItemResponse>>, ServerError> {
    let models = state.store.list_models().await?;
    let download_tasks = state.store.list_tasks(Some("model_download")).await?;

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
        let computed_status = if model.local_path.is_some() {
            ModelListStatus::Downloaded
        } else if pending_task.is_some() {
            ModelListStatus::Pending
        } else {
            ModelListStatus::NotDownloaded
        };

        let include = match q.status {
            ModelListStatus::All => true,
            _ => q.status == computed_status,
        };
        if !include {
            continue;
        }

        items.push(ModelCatalogItemResponse {
            id: model.id,
            display_name: model.display_name,
            repo_id: model.repo_id,
            filename: model.filename,
            backend_ids: model.backend_ids,
            status: computed_status,
            local_path: model.local_path,
            last_downloaded_at: model.last_downloaded_at.map(|v| v.to_rfc3339()),
            pending_task_id: pending_task.map(|t| t.id.clone()),
            pending_task_status: pending_task.map(|t| t.status.clone()),
        });
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
    State(_state): State<Arc<AppState>>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = &req.backend_id;

    validate_path("model_path", &req.model_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest(
            "num_workers must be at least 1".into(),
        ));
    }

    info!(
        backend = %bid,
        model_path = %req.model_path,
        workers = req.num_workers,
        "loading model"
    );

    let backend = Backend::from_str(bid)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {bid}")))?;

    slab_core::api::backend(backend)
        .op(slab_core::api::Event::LoadModel)
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status: "loaded".into(),
    }))
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
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = &req.backend_id;

    info!(backend = %bid, "unloading model");

    let backend = Backend::from_str(bid)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {bid}")))?;

    slab_core::api::backend(backend)
        .op(slab_core::api::Event::UnloadModel)
        .input(slab_core::Payload::default())
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status: "unloaded".into(),
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
    State(_state): State<Arc<AppState>>,
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
    State(_state): State<Arc<AppState>>,
    Json(req): Json<SwitchModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = &req.backend_id;
    validate_path("model_path", &req.model_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest(
            "num_workers must be at least 1".into(),
        ));
    }

    info!(backend = %bid, model_path = %req.model_path, "switching model");

    let backend = Backend::from_str(bid)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {bid}")))?;
    slab_core::api::backend(backend)
        .op(slab_core::api::Event::LoadModel)
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status: "loaded".into(),
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
    State(state): State<Arc<AppState>>,
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

    let configured_target_dir = state
        .store
        .get_config_value(MODEL_TARGET_DIR_CONFIG_KEY)
        .await?
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned);
    let effective_target_dir = configured_target_dir;
    if let Some(dir) = &effective_target_dir {
        validate_path("target_dir", dir)?;
    }

    let backend = Backend::from_str(backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {backend_id}")))?;
    let canonical_backend_id = backend.to_string();

    let model = state
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
        "target_dir": effective_target_dir,
    })
    .to_string();

    state
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

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
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
        let target_dir = input["target_dir"]
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
            let api = if let Some(dir) = target_dir {
                ApiBuilder::new()
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

    state
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
