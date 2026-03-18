use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures::StreamExt;
use utoipa::OpenApi;

use crate::api::v1::chat::schema::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
    ChatReasoningEffort, ChatThinkingConfig, ChatThinkingType,
    ChatVerbosity,
};
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::models::{ChatCompletionOutput, ChatStreamChunk};
use crate::domain::services::ChatService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        OpenAiMessage,
        ChatChoice,
        ChatThinkingConfig,
        ChatThinkingType,
        ChatReasoningEffort,
        ChatVerbosity
    ))
)]
pub struct ChatApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat/completions", post(chat_completions))
}

#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    tag = "chat",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Completion generated", body = ChatCompletionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
async fn chat_completions(
    State(service): State<ChatService>,
    ValidatedJson(req): ValidatedJson<ChatCompletionRequest>,
) -> Result<Response, ServerError> {
    match service.create_chat_completion(req.into()).await? {
        ChatCompletionOutput::Json(response) => {
            Ok(Json(ChatCompletionResponse::from(response)).into_response())
        }
        ChatCompletionOutput::Stream(stream) => {
            let event_stream = stream.map(|chunk| -> Result<Event, Infallible> {
                match chunk {
                    ChatStreamChunk::Data(data) => Ok(Event::default().data(data)),
                    ChatStreamChunk::Comment(comment) => Ok(Event::default().comment(comment)),
                }
            });
            Ok(Sse::new(event_stream).into_response())
        }
    }
}

