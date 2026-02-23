//! Model-management routes.
//!
//! These endpoints allow loading and inspecting AI models at runtime without
//! restarting the server.
//!
//! # Authentication
//!
//! When `SLAB_MANAGEMENT_TOKEN` is set, every request to these routes must
//! include an `Authorization: Bearer <token>` header.  Without the env var,
//! the routes are unauthenticated – suitable only for development or when the
//! network/reverse-proxy layer enforces access control.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{body::Body, Json, Router};
use tracing::info;

use crate::error::ServerError;
use crate::models::management::{LoadModelRequest, ModelStatusResponse, ModelTypePath};
use crate::state::AppState;

/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models/{model_type}/load",   post(load_model))
        .route("/models/{model_type}/status", get(model_status))
        .layer(middleware::from_fn_with_state(
            // State is passed at layer time but the middleware closure receives
            // it per-request via `req.extensions()`.
            Arc::new(()) as Arc<()>,
            // We use a plain async fn instead of a closure to keep lifetimes
            // clear; state is accessed via the axum State extractor.
            |req: Request<Body>, next: Next| async move {
                check_management_auth(req, next).await
            },
        ))
}

/// Optional bearer-token guard for management routes.
///
/// If `SLAB_MANAGEMENT_TOKEN` is set in the environment, every incoming
/// request must supply a matching `Authorization: Bearer <token>` header.
async fn check_management_auth(req: Request<Body>, next: Next) -> Response {
    // Read the expected token from the environment on each request so that
    // hot-reloads (e.g. during development) are reflected immediately.
    let expected = std::env::var("SLAB_MANAGEMENT_TOKEN").ok();

    if let Some(expected_token) = expected {
        let provided = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        match provided {
            Some(token) if token == expected_token => {} // authorised
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({ "error": "unauthorised" })),
                )
                    .into_response();
            }
        }
    }

    next.run(req).await
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

/// Validate that a path string is safe to pass to slab-core.
///
/// Rejects empty strings and paths that contain `..` components to prevent
/// path traversal attacks.  Only absolute paths are accepted.
fn validate_path(label: &str, path: &str) -> Result<(), ServerError> {
    if path.is_empty() {
        return Err(ServerError::BadRequest(format!("{label} must not be empty")));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(ServerError::BadRequest(format!(
            "{label} must be an absolute path (got: {path})"
        )));
    }
    // Reject any path with `..` components to prevent directory traversal.
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

/// Load (or hot-reload) a model (`POST /api/models/{type}/load`).
///
/// `{type}` must be one of `"llama"`, `"whisper"`, or `"diffusion"`.
///
/// Both `lib_path` and `model_path` must be absolute paths.  Relative paths
/// and paths containing `..` are rejected to prevent path traversal.
#[utoipa::path(
    post,
    path = "/api/models/{model_type}/load",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
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
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    // Validate paths to prevent path traversal and loading from unexpected
    // locations.
    validate_path("lib_path",   &req.lib_path)?;
    validate_path("model_path", &req.model_path)?;

    // Ensure at least one worker thread is requested.
    if req.num_workers == 0 {
        return Err(ServerError::BadRequest(
            "num_workers must be at least 1".into(),
        ));
    }

    info!(
        backend    = %bid,
        lib_path   = %req.lib_path,
        model_path = %req.model_path,
        workers    = req.num_workers,
        "loading model"
    );

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
///
/// Returns whether the backend is registered and ready to accept requests.
/// **Note:** `status: "ready"` confirms the backend worker is running, but
/// does **not** guarantee a model file has been loaded.  Use the `load`
/// endpoint and check for errors to confirm full model readiness.
#[utoipa::path(
    get,
    path = "/api/models/{model_type}/status",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
    responses(
        (status = 200, description = "Backend worker is running", body = ModelStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
pub async fn model_status(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    // slab-core does not expose a "is model loaded" query; we report "ready"
    // to indicate the backend worker channel is registered and accepting tasks.
    Ok(Json(ModelStatusResponse {
        backend: bid.to_owned(),
        status:  "ready".into(),
    }))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_path_empty() {
        assert!(validate_path("lib_path", "").is_err());
    }

    #[test]
    fn validate_path_relative() {
        assert!(validate_path("lib_path", "relative/path.so").is_err());
    }

    #[test]
    fn validate_path_traversal() {
        assert!(validate_path("lib_path", "/safe/../../../etc/passwd").is_err());
    }

    #[test]
    fn validate_path_absolute_ok() {
        assert!(validate_path("lib_path", "/usr/lib/libllama.so").is_ok());
    }

    #[test]
    fn backend_id_known() {
        assert_eq!(backend_id("llama"),     Some("ggml.llama"));
        assert_eq!(backend_id("whisper"),   Some("ggml.whisper"));
        assert_eq!(backend_id("diffusion"), Some("ggml.diffusion"));
    }

    #[test]
    fn backend_id_unknown() {
        assert!(backend_id("gpt4").is_none());
    }
}

