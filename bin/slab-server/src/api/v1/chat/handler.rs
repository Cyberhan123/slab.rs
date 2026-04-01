use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use serde_json::json;
use utoipa::OpenApi;

use crate::api::v1::chat::schema::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage,
    ChatContentPart, ChatMessage as OpenAiMessage, ChatMessageContent, ChatModelCapabilities,
    ChatModelOption, ChatModelSource, ChatPromptTokensDetails, ChatReasoningEffort,
    ChatResponseFormat, ChatResponseFormatType, ChatResponseJsonSchema, ChatStreamOptions,
    ChatThinkingConfig, ChatThinkingType, ChatToolCall, ChatToolFunction, ChatVerbosity,
    CompletionChoice, CompletionRequest, CompletionResponse, OpenAiErrorResponse, StopSequences,
};
use crate::api::validation::ValidatedJson;
use crate::error::ServerError;
use slab_app_core::context::AppState;
use slab_app_core::domain::models::{ChatCompletionOutput, ChatStreamChunk, TextCompletionOutput};
use slab_app_core::domain::services::ChatService;

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
    responses(
        (status = 200, description = "Selectable chat model options", body = [ChatModelOption]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_chat_models(
    State(service): State<ChatService>,
) -> Result<Json<Vec<ChatModelOption>>, ServerError> {
    let items = service.list_chat_models().await?.into_iter().map(Into::into).collect();
    Ok(Json(items))
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
    ValidatedJson(req): ValidatedJson<ChatCompletionRequest>,
) -> Response {
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
    ValidatedJson(req): ValidatedJson<CompletionRequest>,
) -> Response {
    match service.create_text_completion(req.into()).await {
        Ok(TextCompletionOutput::Json(response)) => {
            Json(CompletionResponse::from(response)).into_response()
        }
        Ok(TextCompletionOutput::Stream(stream)) => sse_response(stream),
        Err(error) => openai_error_response(error.into()),
    }
}

fn sse_response(stream: futures::stream::BoxStream<'static, ChatStreamChunk>) -> Response {
    let event_stream = stream.map(|chunk| -> Result<Event, Infallible> {
        Ok(Event::default().data(match chunk {
            ChatStreamChunk::Data(data) => data,
        }))
    });
    Sse::new(event_stream).into_response()
}

fn openai_error_response(error: ServerError) -> Response {
    let (status, message, error_type, code, param) = match error {
        ServerError::NotFound(message) => (
            StatusCode::NOT_FOUND,
            message,
            "invalid_request_error".to_owned(),
            Some("not_found".to_owned()),
            None,
        ),
        ServerError::BadRequest(message) => (
            StatusCode::BAD_REQUEST,
            message,
            "invalid_request_error".to_owned(),
            Some("bad_request".to_owned()),
            None,
        ),
        ServerError::BadRequestData { message, data } => (
            StatusCode::BAD_REQUEST,
            message,
            string_field(&data, "error_type").unwrap_or_else(|| "invalid_request_error".to_owned()),
            string_field(&data, "code").or(Some("bad_request".to_owned())),
            string_field(&data, "param"),
        ),
        ServerError::BackendNotReady(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            message,
            "service_unavailable_error".to_owned(),
            Some("backend_not_ready".to_owned()),
            None,
        ),
        ServerError::NotImplemented(message) => (
            StatusCode::NOT_IMPLEMENTED,
            message,
            "invalid_request_error".to_owned(),
            Some("not_implemented".to_owned()),
            None,
        ),
        ServerError::TooManyRequests(message) => (
            StatusCode::TOO_MANY_REQUESTS,
            message,
            "rate_limit_error".to_owned(),
            Some("too_many_requests".to_owned()),
            None,
        ),
        ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_owned(),
            "server_error".to_owned(),
            None,
            None,
        ),
    };

    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "param": param,
                "code": code,
            }
        })),
    )
        .into_response()
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value.get(field).and_then(serde_json::Value::as_str).map(str::to_owned)
}
