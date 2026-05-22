//! Core application error type.
//!
//! [`AppCoreError`] is the unified error type for the slab-app-core library.
//! It deliberately has no HTTP/axum dependency so it can be used both from
//! the HTTP server layer and from native Tauri IPC commands.

use serde::Serialize;
use thiserror::Error;

/// Structured client-facing error details for known bad-request cases.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AppCoreErrorData {
    UnsupportedChatParameter {
        #[serde(rename = "error_type")]
        error_type: &'static str,
        param: String,
    },
}

impl AppCoreErrorData {
    pub fn unsupported_chat_parameter(param: impl Into<String>) -> Self {
        Self::UnsupportedChatParameter { error_type: "invalid_request_error", param: param.into() }
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            Self::UnsupportedChatParameter { error_type, .. } => error_type,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedChatParameter { .. } => "unsupported_chat_parameter",
        }
    }

    pub fn param(&self) -> &str {
        match self {
            Self::UnsupportedChatParameter { param, .. } => param,
        }
    }
}

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
    BadRequestData { message: String, data: AppCoreErrorData },

    /// Backend not initialized or ready.
    #[error("backend not ready: {0}")]
    BackendNotReady(String),

    /// Runtime reported resource pressure while loading or running a model.
    #[error("runtime memory pressure: {0}")]
    RuntimeMemoryPressure(String),

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

impl From<slab_config::ConfigError> for AppCoreError {
    fn from(error: slab_config::ConfigError) -> Self {
        match error {
            slab_config::ConfigError::NotFound(message) => Self::NotFound(message),
            slab_config::ConfigError::BadRequest(message) => Self::BadRequest(message),
            slab_config::ConfigError::NotImplemented(message) => Self::NotImplemented(message),
            slab_config::ConfigError::Internal(message) => Self::Internal(message),
        }
    }
}
