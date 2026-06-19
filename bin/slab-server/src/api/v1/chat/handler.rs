use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use utoipa::OpenApi;

use crate::api::v1::chat::schema::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
    ChatContentPart, ChatMessage as OpenAiMessage, ChatMessageContent, ChatModelCapabilities,
    ChatModelOption, ChatModelSource, ChatPromptTokensDetails, ChatReasoningEffort,
    ChatResponseFormat, ChatResponseFormatType, ChatResponseJsonSchema, ChatStreamOptions,
    ChatThinkingConfig, ChatThinkingType, ChatToolCall, ChatToolFunction, ChatVerbosity,
    CompletionChoice, CompletionRequest, CompletionResponse, OpenAiError, OpenAiErrorResponse,
    StopSequences,
};
use crate::api::validation::ValidatedJson;
use crate::error::{ServerError, message_i18n, message_i18n_with_detail};
use slab_app_core::context::AppState;
use slab_app_core::domain::models::{ChatCompletionOutput, ChatStreamChunk, TextCompletionOutput};
use slab_app_core::domain::services::{ChatService, ModelService};
use slab_types::ServerI18nKey;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions, completions, list_chat_models),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        CompletionRequest,
        CompletionResponse,
        ChatCompletionUsage,
        ChatModelCapabilities,
        ChatModelOption,
        ChatModelSource,
        ChatContentPart,
        ChatMessageContent,
        OpenAiMessage,
        ChatChoice,
        CompletionChoice,
        ChatPromptTokensDetails,
        ChatThinkingConfig,
        ChatThinkingType,
        ChatReasoningEffort,
        ChatVerbosity,
        ChatResponseFormat,
        ChatResponseFormatType,
        ChatResponseJsonSchema,
        ChatStreamOptions,
        StopSequences,
        ChatToolCall,
        ChatToolFunction,
        OpenAiErrorResponse
    ))
)]
pub struct ChatApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat/models", get(list_chat_models))
        .route("/chat/completions", post(chat_completions))
        .route("/completions", post(completions))
}

#[utoipa::path(
    get,
    path = "/v1/chat/models",
    tag = "chat",
    summary = "Deprecated chat model listing compatibility route",
    description = "Compatibility wrapper over GET /v1/models filtered by capability=chat_generation.",
    responses(
        (status = 200, description = "Selectable chat model options", body = [ChatModelOption]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_chat_models(
    State(service): State<ModelService>,
) -> Result<impl IntoResponse, ServerError> {
    let items: Vec<ChatModelOption> =
        service.list_chat_models().await?.into_iter().map(Into::into).collect();
    Ok((
        [
            (HeaderName::from_static("deprecation"), HeaderValue::from_static("true")),
            (
                HeaderName::from_static("sunset"),
                HeaderValue::from_static("Tue, 08 Jun 2027 00:00:00 GMT"),
            ),
        ],
        Json(items),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    tag = "chat",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Completion generated", body = ChatCompletionResponse),
        (status = 400, description = "Bad request", body = OpenAiErrorResponse),
        (status = 500, description = "Backend error", body = OpenAiErrorResponse),
    )
)]
async fn chat_completions(
    State(service): State<ChatService>,
    req: Result<ValidatedJson<ChatCompletionRequest>, ServerError>,
) -> Response {
    let ValidatedJson(req) = match req {
        Ok(req) => req,
        Err(error) => return openai_error_response(error),
    };

    match service.create_chat_completion(req.into()).await {
        Ok(ChatCompletionOutput::Json(response)) => {
            Json(ChatCompletionResponse::from(response)).into_response()
        }
        Ok(ChatCompletionOutput::Stream(stream)) => sse_response(stream),
        Err(error) => openai_error_response(error.into()),
    }
}

#[utoipa::path(
    post,
    path = "/v1/completions",
    tag = "chat",
    request_body = CompletionRequest,
    responses(
        (status = 200, description = "Text completion generated", body = CompletionResponse),
        (status = 400, description = "Bad request", body = OpenAiErrorResponse),
        (status = 500, description = "Backend error", body = OpenAiErrorResponse),
    )
)]
async fn completions(
    State(service): State<ChatService>,
    req: Result<ValidatedJson<CompletionRequest>, ServerError>,
) -> Response {
    let ValidatedJson(req) = match req {
        Ok(req) => req,
        Err(error) => return openai_error_response(error),
    };

    match service.create_text_completion(req.into()).await {
        Ok(TextCompletionOutput::Json(response)) => {
            Json(CompletionResponse::from(response)).into_response()
        }
        Ok(TextCompletionOutput::Stream(stream)) => sse_response(stream),
        Err(error) => openai_error_response(error.into()),
    }
}

fn sse_response(stream: futures::stream::BoxStream<'static, ChatStreamChunk>) -> Response {
    let event_stream =
        stream.map(|chunk| -> Result<Event, Infallible> { Ok(Event::default().data(chunk)) });
    Sse::new(event_stream).into_response()
}

fn openai_error_response(error: ServerError) -> Response {
    let (status, message, error_type, code, param, i18n) = match error {
        ServerError::NotFound(message) => (
            StatusCode::NOT_FOUND,
            message.clone(),
            "invalid_request_error".to_owned(),
            Some("not_found".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorNotFound, &message)),
        ),
        ServerError::BadRequest(message) => (
            StatusCode::BAD_REQUEST,
            message.clone(),
            "invalid_request_error".to_owned(),
            Some("bad_request".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, &message)),
        ),
        ServerError::BadRequestData { message, data } => (
            StatusCode::BAD_REQUEST,
            message.clone(),
            data.error_type().to_owned(),
            Some(data.code().to_owned()),
            Some(data.param().to_owned()),
            Some(message_i18n_with_detail(ServerI18nKey::ErrorBadRequest, &message)),
        ),
        ServerError::RequestValidationFailed(message) => (
            StatusCode::BAD_REQUEST,
            message.clone(),
            "invalid_request_error".to_owned(),
            Some("bad_request".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorRequestValidationFailed, &message)),
        ),
        ServerError::Conflict(message) => (
            StatusCode::CONFLICT,
            message.clone(),
            "invalid_request_error".to_owned(),
            Some("conflict".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorConflict, &message)),
        ),
        ServerError::BackendNotReady(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            message.clone(),
            "service_unavailable_error".to_owned(),
            Some("backend_not_ready".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorBackendNotReady, &message)),
        ),
        ServerError::RuntimeFailure { message, data } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            message.clone(),
            data.error_type().to_owned(),
            data.runtime_code().map(str::to_owned),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorRuntimeError, &message)),
        ),
        ServerError::NotImplemented(message) => (
            StatusCode::NOT_IMPLEMENTED,
            message.clone(),
            "invalid_request_error".to_owned(),
            Some("not_implemented".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorNotImplemented, &message)),
        ),
        ServerError::TooManyRequests(message) => (
            StatusCode::TOO_MANY_REQUESTS,
            message.clone(),
            "rate_limit_error".to_owned(),
            Some("too_many_requests".to_owned()),
            None,
            Some(message_i18n_with_detail(ServerI18nKey::ErrorTooManyRequests, &message)),
        ),
        ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_owned(),
            "server_error".to_owned(),
            None,
            None,
            Some(message_i18n(ServerI18nKey::ErrorInternalError)),
        ),
    };

    (
        status,
        Json(OpenAiErrorResponse { error: OpenAiError { message, error_type, param, code, i18n } }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use serde_json::Value;

    use super::{ServerError, openai_error_response};

    #[tokio::test]
    async fn openai_error_response_includes_nested_message_i18n() {
        let response = openai_error_response(ServerError::BadRequest("model is required".into()));

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("read body");
        let payload: Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["error"]["message"], "model is required");
        assert_eq!(payload["error"]["i18n"]["message"]["key"], "server.errors.badRequest");
        assert_eq!(payload["error"]["i18n"]["message"]["params"]["detail"], "model is required");
    }

    #[tokio::test]
    async fn openai_validation_error_response_uses_validation_i18n_key() {
        let response =
            openai_error_response(ServerError::RequestValidationFailed("model: required".into()));

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.expect("read body");
        let payload: Value = serde_json::from_slice(&body).expect("json body");

        assert_eq!(payload["error"]["message"], "model: required");
        assert_eq!(
            payload["error"]["i18n"]["message"]["key"],
            "server.errors.requestValidationFailed"
        );
        assert_eq!(payload["error"]["i18n"]["message"]["params"]["detail"], "model: required");
    }
}
