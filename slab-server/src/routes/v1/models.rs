//! Model-management routes.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use tracing::{info, warn};
use utoipa::OpenApi;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::schemas::v1::models::{
    DownloadModelRequest, ListAvailableQuery, LoadModelRequest, ModelStatusResponse,
    SwitchModelRequest,
};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(load_model, unload_model, switch_model, download_model),
    components(schemas(
        LoadModelRequest,
        ModelStatusResponse,
        SwitchModelRequest,
        DownloadModelRequest,
        ListAvailableQuery
    ))
)]
pub struct ModelsApi;
/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
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

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Load (or hot-reload) a model (`POST /api/models/load`).
#[utoipa::path(
    post,
    path = "/v1/models/load",
    tag = "v1::models",
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model load initiated",   body = ModelStatusResponse),
        (status = 400, description = "Unknown model type or invalid paths"),
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
        backend    = %bid,
        // lib_path   = %req.lib_path,
        model_path = %req.model_path,
        workers    = req.num_workers,
        "loading model"
    );

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            // "lib_path":    req.lib_path,
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
    tag = "v1::models",
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

    slab_core::api::backend(bid)
        .op("model.unload")
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
///
/// Uses the `hf-hub` sync API (wrapped in `spawn_blocking`) to fetch repo metadata
/// and returns a list of filenames in the repo.
#[utoipa::path(
        get,
        path = "/v1/models/available",
        tag = "v1::models",
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
        use hf_hub::api::sync::Api;
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
    tag = "v1::models",
    params(ListAvailableQuery),
    responses(
        (status = 200, description = "List of available files", body = serde_json::Value),
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

    // // Resolve lib_path: use request value or fall back to configured lib dir.
    // let lib_path = req.lib_path.clone().or_else(|| {
    //     slab_core::api::lib_dirs().and_then(|dirs| match model_type.as_str() {
    //         "llama"     => dirs.llama.clone(),
    //         "whisper"   => dirs.whisper.clone(),
    //         "diffusion" => dirs.diffusion.clone(),
    //         _           => None,
    //     })
    // }).ok_or_else(|| {
    //     ServerError::BadRequest("lib_path required when lib dirs are not configured".into())
    // })?;

    // validate_path("lib_path", &lib_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest(
            "num_workers must be at least 1".into(),
        ));
    }

    info!(backend = %bid, model_path = %req.model_path, "switching model");

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            // "lib_path":    lib_path,
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

/// Download a model weight file from HuggingFace (`POST /api/models/{type}/download`).
///
/// Uses the `hf-hub` sync API (wrapped in `spawn_blocking`) to fetch the file
/// from the given HuggingFace repo.  Returns the local cache path where the
/// file was stored.  The download is async; poll `GET /api/tasks/{id}` for status.
#[utoipa::path(
    post,
    path = "/v1/models/download",
    tag = "v1::models",
    params(ListAvailableQuery),
    request_body = DownloadModelRequest,
    responses(
        (status = 200, description = "List of available files", body = serde_json::Value),
        (status = 400, description = "Bad request (invalid parameters)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn download_model(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DownloadModelRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if req.repo_id.is_empty() {
        return Err(ServerError::BadRequest("repo_id must not be empty".into()));
    }
    if req.filename.is_empty() {
        return Err(ServerError::BadRequest("filename must not be empty".into()));
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let input_data = serde_json::json!({
        "backend_id": req.backend_id,
        "repo_id":    req.repo_id,
        "filename":   req.filename,
        "target_dir": req.target_dir,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "model_download".into(),
            status: "pending".into(),
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

        let repo_id = input["repo_id"].as_str().unwrap_or("").to_owned();
        let filename = input["filename"].as_str().unwrap_or("").to_owned();
        let target_dir = input["target_dir"].as_str().map(str::to_owned);

        // Fail early if required fields are missing or empty.
        if repo_id.is_empty() || filename.is_empty() {
            warn!(task_id = %tid, "model_download task is missing repo_id or filename in stored input_data");
            store
                .update_task_status(
                    &tid,
                    "failed",
                    None,
                    Some("missing repo_id or filename in stored input_data"),
                )
                .await
                .ok();
            task_manager.remove(&tid);
            return;
        }

        // Validate target_dir to prevent directory traversal.
        if let Some(dir) = &target_dir {
            if let Err(e) = validate_path("target_dir", dir) {
                warn!(task_id = %tid, error = %e, "invalid target_dir in model_download task");
                store
                    .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                    .await
                    .ok();
                task_manager.remove(&tid);
                return;
            }
        }

        let result = tokio::task::spawn_blocking(move || {
            use hf_hub::api::sync::{Api, ApiBuilder};
            // If a custom target_dir was provided, configure hf-hub to use it as cache root.
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
    info!(task_id = %task_id, backend_id = %req.backend_id, "model download task accepted");
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}
