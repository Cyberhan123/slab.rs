//! Core application error type.
//!
//! [`AppCoreError`] is the unified error type for the slab-app-core library.
//! It deliberately has no HTTP/axum dependency so it can be used both from
//! the HTTP server layer and from native Tauri IPC commands.

use thiserror::Error;

/// All errors that can occur in the slab-app-core business logic.
#[derive(Debug, Error)]
pub enum AppCoreError {
    /// Propagated from slab-runtime-core's AI runtime.
    #[error("runtime error: {0}")]
    Runtime(#[from] slab_runtime_core::CoreError),

    /// Propagated from the SQLite (or other) store.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// The caller referenced a resource that does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// The caller sent an invalid or malformed request.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// The caller sent an invalid or malformed request with structured details.
    #[error("bad request: {message}")]
    BadRequestData { message: String, data: serde_json::Value },

    /// Backend not initialized or ready.
    #[error("backend not ready: {0}")]
    BackendNotReady(String),

    /// The requested operation is not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Rate limit or concurrency cap exceeded.
    #[error("too many requests: {0}")]
    TooManyRequests(String),

    /// An unclassified internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for AppCoreError {
    fn from(e: anyhow::Error) -> Self {
        AppCoreError::Internal(e.to_string())
    }
}
