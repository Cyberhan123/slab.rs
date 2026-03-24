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
    ChatPromptTokensDetails, ChatReasoningEffort, ChatStreamOptions, ChatThinkingConfig,
    ChatThinkingType, ChatToolCall, ChatToolFunction, ChatVerbosity, OpenAiErrorResponse,
};
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::models::ChatCompletionOutput;
use crate::domain::services::ChatService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions, list_chat_models),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        ChatCompletionUsage,
        ChatModelOption,
        ChatModelSource,
        ChatContentPart,
        ChatMessageContent,
        OpenAiMessage,
        ChatChoice,
        ChatPromptTokensDetails,
        ChatThinkingConfig,
        ChatThinkingType,
        ChatReasoningEffort,
        ChatVerbosity,
        ChatStreamOptions,
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
        Ok(ChatCompletionOutput::Stream(stream)) => {
            let event_stream = stream.map(|chunk| -> Result<Event, Infallible> {
                Ok(Event::default().data(match chunk {
                    crate::domain::models::ChatStreamChunk::Data(data) => data,
                }))
            });
            Sse::new(event_stream).into_response()
        }
        Err(error) => openai_error_response(error),
    }
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
