//! Unified server error type.
//!
//! Every handler returns `Result<T, ServerError>`, which implements
//! [`axum::response::IntoResponse`] so errors are automatically converted
//! to a JSON-body HTTP response with an appropriate status code.
//!
//! **Security note:** Internal errors (Runtime, Database) are logged with full
//! detail but only a generic message is returned to the caller so that
//! file paths, SQL, or other implementation details never leak to clients.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use tracing::error;
use validator::{ValidationErrors, ValidationErrorsKind};

/// Standard error response format
#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    data: Option<serde_json::Value>,
    message: String,
}

/// Error codes for different error types
mod error_codes {
    pub const NOT_FOUND: u16 = 4004;
    pub const BAD_REQUEST: u16 = 4000;
    pub const BACKEND_NOT_READY: u16 = 5003;
    pub const RUNTIME_ERROR: u16 = 5000;
    pub const DATABASE_ERROR: u16 = 5001;
    pub const INTERNAL_ERROR: u16 = 5002;
    pub const NOT_IMPLEMENTED: u16 = 5010;
    pub const TOO_MANY_REQUESTS: u16 = 4029;
}

/// All errors that can occur in the slab-server request lifecycle.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Propagated from slab-core's AI runtime.
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

    /// An unclassified internal server error.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, code, data, message) = match &self {
            // Client-facing errors: expose the message directly.
            ServerError::NotFound(m) => (
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                None as Option<serde_json::Value>,
                m.clone(),
            ),
            ServerError::BadRequest(m) => {
                (StatusCode::BAD_REQUEST, error_codes::BAD_REQUEST, None, m.clone())
            }
            ServerError::BadRequestData { message, data } => (
                StatusCode::BAD_REQUEST,
                error_codes::BAD_REQUEST,
                Some(data.clone()),
                message.clone(),
            ),
            ServerError::BackendNotReady(m) => {
                (StatusCode::SERVICE_UNAVAILABLE, error_codes::BACKEND_NOT_READY, None, m.clone())
            }

            ServerError::NotImplemented(m) => {
                (StatusCode::NOT_IMPLEMENTED, error_codes::NOT_IMPLEMENTED, None, m.clone())
            }

            ServerError::TooManyRequests(m) => {
                (StatusCode::TOO_MANY_REQUESTS, error_codes::TOO_MANY_REQUESTS, None, m.clone())
            }

            // Internal errors: log the full detail, return a helpful message
            // for common errors while keeping sensitive details private.
            ServerError::Runtime(e) => {
                error!(error = %e, "AI runtime error");
                let message = match e {
                    slab_runtime_core::CoreError::QueueFull { .. }
                    | slab_runtime_core::CoreError::Busy { .. } => {
                        "inference backend is busy".to_owned()
                    }
                    slab_runtime_core::CoreError::BackendShutdown => {
                        "inference backend is unavailable".to_owned()
                    }
                    slab_runtime_core::CoreError::UnsupportedOperation { .. } => {
                        "requested runtime operation is not supported".to_owned()
                    }
                    slab_runtime_core::CoreError::DriverNotRegistered { .. } => {
                        "inference backend is not registered".to_owned()
                    }
                    _ => "inference backend error".to_owned(),
                };
                (StatusCode::INTERNAL_SERVER_ERROR, error_codes::RUNTIME_ERROR, None, message)
            }
            ServerError::Database(e) => {
                error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::DATABASE_ERROR,
                    None,
                    "internal server error".to_owned(),
                )
            }
            ServerError::Internal(m) => {
                error!(message = %m, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::INTERNAL_ERROR,
                    None,
                    "internal server error".to_owned(),
                )
            }
        };

        let error_response = ErrorResponse { code, data, message };

        (status, Json(error_response)).into_response()
    }
}

impl From<slab_app_core::error::AppCoreError> for ServerError {
    fn from(e: slab_app_core::error::AppCoreError) -> Self {
        match e {
            slab_app_core::error::AppCoreError::Runtime(e) => ServerError::Runtime(e),
            slab_app_core::error::AppCoreError::Database(e) => ServerError::Database(e),
            slab_app_core::error::AppCoreError::NotFound(m) => ServerError::NotFound(m),
            slab_app_core::error::AppCoreError::BadRequest(m) => ServerError::BadRequest(m),
            slab_app_core::error::AppCoreError::BadRequestData { message, data } => {
                ServerError::BadRequestData { message, data }
            }
            slab_app_core::error::AppCoreError::BackendNotReady(m) => {
                ServerError::BackendNotReady(m)
            }
            slab_app_core::error::AppCoreError::NotImplemented(m) => ServerError::NotImplemented(m),
            slab_app_core::error::AppCoreError::TooManyRequests(m) => {
                ServerError::TooManyRequests(m)
            }
            slab_app_core::error::AppCoreError::Internal(m) => ServerError::Internal(m),
        }
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

impl From<ValidationErrors> for ServerError {
    fn from(errors: ValidationErrors) -> Self {
        ServerError::BadRequest(format_validation_errors(&errors))
    }
}

fn format_validation_errors(errors: &ValidationErrors) -> String {
    let mut messages = Vec::new();
    collect_validation_messages("", errors, &mut messages);

    if messages.is_empty() { "request validation failed".to_owned() } else { messages.join("; ") }
}

fn collect_validation_messages(
    prefix: &str,
    errors: &ValidationErrors,
    messages: &mut Vec<String>,
) {
    for (field, kind) in errors.errors() {
        let field_path =
            if prefix.is_empty() { field.to_string() } else { format!("{prefix}.{field}") };

        match kind {
            ValidationErrorsKind::Field(field_errors) => {
                for error in field_errors {
                    let message = error
                        .message
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| error.code.to_string());
                    messages.push(format!("{field_path}: {message}"));
                }
            }
            ValidationErrorsKind::Struct(nested) => {
                collect_validation_messages(&field_path, nested, messages);
            }
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    collect_validation_messages(
                        &format!("{field_path}[{index}]"),
                        nested,
                        messages,
                    );
                }
            }
        }
    }
}
