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
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatCompletionUsage, ChatContentPart,
    ChatMessage as OpenAiMessage, ChatMessageContent, ChatModelOption, ChatModelSource,
    ChatPromptTokensDetails, ChatReasoningEffort, ChatResponseFormat, ChatResponseFormatType,
    ChatResponseJsonSchema, ChatStreamOptions, ChatThinkingConfig, ChatThinkingType, ChatToolCall,
    ChatToolFunction, ChatVerbosity, CompletionChoice, CompletionRequest, CompletionResponse,
    OpenAiErrorResponse, StopSequences,
};
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::models::{ChatCompletionOutput, ChatStreamChunk, TextCompletionOutput};
use crate::domain::services::ChatService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions, completions, list_chat_models),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        CompletionRequest,
        CompletionResponse,
        ChatCompletionUsage,
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
        Err(error) => openai_error_response(error),
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
        Err(error) => openai_error_response(error),
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
    let (status, message, error_type, code) = match error {
        ServerError::NotFound(message) => (
            StatusCode::NOT_FOUND,
            message,
            "invalid_request_error",
            Some("not_found"),
        ),
        ServerError::BadRequest(message) => (
            StatusCode::BAD_REQUEST,
            message,
            "invalid_request_error",
            Some("bad_request"),
        ),
        ServerError::BadRequestData { message, .. } => (
            StatusCode::BAD_REQUEST,
            message,
            "invalid_request_error",
            Some("bad_request"),
        ),
        ServerError::BackendNotReady(message) => (
            StatusCode::SERVICE_UNAVAILABLE,
            message,
            "service_unavailable_error",
            Some("backend_not_ready"),
        ),
        ServerError::NotImplemented(message) => (
            StatusCode::NOT_IMPLEMENTED,
            message,
            "invalid_request_error",
            Some("not_implemented"),
        ),
        ServerError::Runtime(_) | ServerError::Database(_) | ServerError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal server error".to_owned(),
            "server_error",
            None,
        ),
    };

    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "param": serde_json::Value::Null,
                "code": code,
            }
        })),
    )
        .into_response()
}
