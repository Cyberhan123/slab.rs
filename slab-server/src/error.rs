//! Unified server error type.
//!
//! Every handler returns `Result<T, ServerError>`, which implements
//! [`axum::response::IntoResponse`] so errors are automatically converted
//! to a JSON-body HTTP response with an appropriate status code.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

/// All errors that can occur in the slab-server request lifecycle.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Propagated from slab-core's AI runtime.
    #[error("runtime error: {0}")]
    Runtime(#[from] slab_core::RuntimeError),

    /// Propagated from the SQLite (or other) store.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// The caller referenced a resource that does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// The caller sent an invalid or malformed request.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// An unclassified internal server error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ServerError::NotFound(m)   => (StatusCode::NOT_FOUND, m.clone()),
            ServerError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ServerError::Runtime(e)    => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServerError::Database(e)   => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServerError::Internal(m)   => (StatusCode::INTERNAL_SERVER_ERROR, m.clone()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(e: anyhow::Error) -> Self {
        ServerError::Internal(e.to_string())
    }
}
