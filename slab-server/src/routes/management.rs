//! Model-management routes.
//!
//! These endpoints allow loading and inspecting AI models at runtime without
//! restarting the server.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use tracing::info;

use crate::error::ServerError;
use crate::models::management::{LoadModelRequest, ModelStatusResponse, ModelTypePath};
use crate::state::AppState;

/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models/{model_type}/load",   post(load_model))
        .route("/models/{model_type}/status", get(model_status))
}

/// Map a user-facing model-type string to a slab-core backend identifier.
fn backend_id(model_type: &str) -> Option<&'static str> {
    match model_type {
        "llama"     => Some("ggml.llama"),
        "whisper"   => Some("ggml.whisper"),
        "diffusion" => Some("ggml.diffusion"),
        _           => None,
    }
}

/// Load (or hot-reload) a model (`POST /api/models/{type}/load`).
///
/// `{type}` must be one of `"llama"`, `"whisper"`, or `"diffusion"`.
#[utoipa::path(
    post,
    path = "/api/models/{model_type}/load",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model load initiated",  body = ModelStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn load_model(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    info!(backend = %bid, lib_path = %req.lib_path, model_path = %req.model_path, "loading model");

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            "lib_path":    req.lib_path,
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status:  "loaded".into(),
    }))
}

/// Get the current status of a model backend (`GET /api/models/{type}/status`).
#[utoipa::path(
    get,
    path = "/api/models/{model_type}/status",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
    responses(
        (status = 200, description = "Model status", body = ModelStatusResponse),
        (status = 400, description = "Unknown model type"),
    )
)]
pub async fn model_status(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status:  "ready".into(),
    }))
}
