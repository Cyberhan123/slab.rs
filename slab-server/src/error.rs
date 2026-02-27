//! Unified server error type.
//!
//! Every handler returns `Result<T, ServerError>`, which implements
//! [`axum::response::IntoResponse`] so errors are automatically converted
//! to a JSON-body HTTP response with an appropriate status code.
//!
//! **Security note:** Internal errors (Runtime, Database) are logged with full
//! detail but only a generic message is returned to the caller so that
//! file paths, SQL, or other implementation details never leak to clients.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;
use tracing::error;

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

    /// Backend not initialized or ready.
    #[error("backend not ready: {0}")]
    BackendNotReady(String),

    /// An unclassified internal server error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, client_message) = match &self {
            // Client-facing errors: expose the message directly.
            ServerError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            ServerError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ServerError::BackendNotReady(m) => (StatusCode::SERVICE_UNAVAILABLE, m.clone()),

            // Internal errors: log the full detail, return a helpful message
            // for common errors while keeping sensitive details private.
            ServerError::Runtime(e) => {
                error!(error = %e, "AI runtime error");
                let message = match e {
                    slab_core::RuntimeError::NotInitialized => {
                        "Backend not initialized. Please ensure the Whisper library and model are loaded. \
                        Set SLAB_WHISPER_LIB_DIR environment variable or use POST /admin/backends/reload".to_owned()
                    }
                    slab_core::RuntimeError::LibraryLoadFailed { backend, .. } => {
                        format!("{} library failed to load. Check SLAB_{}_LIB_DIR environment variable.", \
                            backend, backend.to_uppercase().replace(".", "_"))
                    }
                    _ => "inference backend error".to_owned()
                };
                (StatusCode::INTERNAL_SERVER_ERROR, message)
            }
            ServerError::Database(e) => {
                error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_owned(),
                )
            }
            ServerError::Internal(m) => {
                error!(message = %m, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal server error".to_owned(),
                )
            }
        };
        (status, Json(json!({ "error": client_message }))).into_response()
    }
}

impl From<anyhow::Error> for ServerError {
    fn from(e: anyhow::Error) -> Self {
        // Log the full error chain (including backtrace if available) before
        // discarding it so that diagnostic detail is preserved in the server
        // logs even though clients only see a generic message.
        error!(error = ?e, "converting anyhow error to ServerError::Internal");
        ServerError::Internal(e.to_string())
    }
}
