//! Unified server error type.
//!
//! Every handler returns `Result<T, ServerError>`, which implements
//! [`axum::response::IntoResponse`] so errors are automatically converted
//! to a JSON-body HTTP response with an appropriate status code.
//!
//! **Security note:** Internal errors (Runtime, Database) are logged with full
//! detail but only a generic message is returned to the caller so that
//! file paths, SQL, or other implementation details never leak to clients.

use std::collections::BTreeMap;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;
use slab_app_core::error::AppCoreErrorData;
use slab_types::{I18nPayload, ServerI18nKey};
use thiserror::Error;
use tracing::error;
use validator::{ValidationErrors, ValidationErrorsKind};

/// Standard error response format
#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    data: Option<AppCoreErrorData>,
    message: String,
    #[serde(skip_serializing_if = "I18nPayload::is_empty")]
    i18n: I18nPayload,
}

/// Error codes for different error types
mod error_codes {
    pub const NOT_FOUND: u16 = 4004;
    pub const BAD_REQUEST: u16 = 4000;
    pub const CONFLICT: u16 = 4009;
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
    BadRequestData { message: String, data: AppCoreErrorData },

    /// The caller sent a syntactically valid request that failed schema validation.
    #[error("request validation failed: {0}")]
    RequestValidationFailed(String),

    /// The request conflicts with the current resource state.
    #[error("conflict: {0}")]
    Conflict(String),

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

impl ServerError {
    pub(crate) fn agent_code_message(self) -> (&'static str, String, I18nPayload) {
        match self {
            ServerError::NotFound(message) => (
                "not_found",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorNotFound, &message),
            ),
            ServerError::BadRequest(message) => (
                "bad_request",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, &message),
            ),
            ServerError::BadRequestData { message, .. } => (
                "bad_request",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, &message),
            ),
            ServerError::RequestValidationFailed(message) => (
                "bad_request",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorRequestValidationFailed, &message),
            ),
            ServerError::Conflict(message) => (
                "conflict",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorConflict, &message),
            ),
            ServerError::BackendNotReady(message) => (
                "backend_not_ready",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBackendNotReady, &message),
            ),
            ServerError::NotImplemented(message) => (
                "not_implemented",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorNotImplemented, &message),
            ),
            ServerError::TooManyRequests(message) => (
                "too_many_requests",
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorTooManyRequests, &message),
            ),
            ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => (
                "internal_error",
                "internal server error".to_owned(),
                message_i18n(ServerI18nKey::ErrorInternalError),
            ),
        }
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, code, data, message, i18n) = match &self {
            // Client-facing errors: expose the message directly.
            ServerError::NotFound(m) => (
                StatusCode::NOT_FOUND,
                error_codes::NOT_FOUND,
                None as Option<AppCoreErrorData>,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorNotFound, m),
            ),
            ServerError::BadRequest(m) => (
                StatusCode::BAD_REQUEST,
                error_codes::BAD_REQUEST,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, m),
            ),
            ServerError::BadRequestData { message, data } => (
                StatusCode::BAD_REQUEST,
                error_codes::BAD_REQUEST,
                Some(data.clone()),
                message.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, message),
            ),
            ServerError::RequestValidationFailed(m) => (
                StatusCode::BAD_REQUEST,
                error_codes::BAD_REQUEST,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorRequestValidationFailed, m),
            ),
            ServerError::Conflict(m) => (
                StatusCode::CONFLICT,
                error_codes::CONFLICT,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorConflict, m),
            ),
            ServerError::BackendNotReady(m) => (
                StatusCode::SERVICE_UNAVAILABLE,
                error_codes::BACKEND_NOT_READY,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorBackendNotReady, m),
            ),

            ServerError::NotImplemented(m) => (
                StatusCode::NOT_IMPLEMENTED,
                error_codes::NOT_IMPLEMENTED,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorNotImplemented, m),
            ),

            ServerError::TooManyRequests(m) => (
                StatusCode::TOO_MANY_REQUESTS,
                error_codes::TOO_MANY_REQUESTS,
                None,
                m.clone(),
                message_i18n_with_detail(ServerI18nKey::ErrorTooManyRequests, m),
            ),

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
                let key = match e {
                    slab_runtime_core::CoreError::QueueFull { .. }
                    | slab_runtime_core::CoreError::Busy { .. } => ServerI18nKey::ErrorRuntimeBusy,
                    slab_runtime_core::CoreError::BackendShutdown => {
                        ServerI18nKey::ErrorRuntimeUnavailable
                    }
                    slab_runtime_core::CoreError::UnsupportedOperation { .. } => {
                        ServerI18nKey::ErrorRuntimeUnsupportedOperation
                    }
                    slab_runtime_core::CoreError::DriverNotRegistered { .. } => {
                        ServerI18nKey::ErrorRuntimeDriverNotRegistered
                    }
                    _ => ServerI18nKey::ErrorRuntimeError,
                };
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::RUNTIME_ERROR,
                    None,
                    message,
                    message_i18n(key),
                )
            }
            ServerError::Database(e) => {
                error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::DATABASE_ERROR,
                    None,
                    "internal server error".to_owned(),
                    message_i18n(ServerI18nKey::ErrorDatabaseError),
                )
            }
            ServerError::Internal(m) => {
                error!(message = %m, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::INTERNAL_ERROR,
                    None,
                    "internal server error".to_owned(),
                    message_i18n(ServerI18nKey::ErrorInternalError),
                )
            }
        };

        let error_response = ErrorResponse { code, data, message, i18n };

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
            slab_app_core::error::AppCoreError::Conflict(m) => ServerError::Conflict(m),
            slab_app_core::error::AppCoreError::BackendNotReady(m) => {
                ServerError::BackendNotReady(m)
            }
            slab_app_core::error::AppCoreError::RuntimeMemoryPressure(m) => {
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
        ServerError::RequestValidationFailed(format_validation_errors(&errors))
    }
}

fn format_validation_errors(errors: &ValidationErrors) -> String {
    let mut messages = Vec::new();
    collect_validation_messages("", errors, &mut messages);

    if messages.is_empty() { "request validation failed".to_owned() } else { messages.join("; ") }
}

pub(crate) fn message_i18n(key: ServerI18nKey) -> I18nPayload {
    I18nPayload::with_field("message", key)
}

pub(crate) fn message_i18n_with_detail(key: ServerI18nKey, detail: &str) -> I18nPayload {
    I18nPayload::with_field_params(
        "message",
        key,
        BTreeMap::from([("detail".to_owned(), Value::String(detail.to_owned()))]),
    )
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

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use serde_json::Value;
    use slab_app_core::error::AppCoreErrorData;

    use super::ServerError;

    #[tokio::test]
    async fn error_response_includes_message_i18n_payload() {
        let response = ServerError::BadRequest("model is required".to_owned()).into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("read body");
        let payload: Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["message"], "model is required");
        assert_eq!(payload["i18n"]["message"]["key"], "server.errors.badRequest");
        assert_eq!(payload["i18n"]["message"]["params"]["detail"], "model is required");
    }

    #[tokio::test]
    async fn validation_error_response_uses_validation_i18n_key() {
        let response =
            ServerError::RequestValidationFailed("model: required".to_owned()).into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("read body");
        let payload: Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["message"], "model: required");
        assert_eq!(payload["i18n"]["message"]["key"], "server.errors.requestValidationFailed");
        assert_eq!(payload["i18n"]["message"]["params"]["detail"], "model: required");
    }

    #[tokio::test]
    async fn bad_request_data_response_preserves_stable_code_and_suggestion() {
        let response = ServerError::BadRequestData {
            message: "model local-qwen cannot be downloaded: missing repo_id. Add a repo_id."
                .to_owned(),
            data: AppCoreErrorData::model_download_unavailable(
                "local-qwen",
                "missing repo_id",
                "Add a repo_id.",
            ),
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("read body");
        let payload: Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["data"]["code"], "model_download_unavailable");
        assert_eq!(payload["data"]["param"], "model_id");
        assert_eq!(payload["data"]["model_id"], "local-qwen");
        assert_eq!(payload["data"]["reason"], "missing repo_id");
        assert_eq!(payload["data"]["suggestion"], "Add a repo_id.");
    }
}
