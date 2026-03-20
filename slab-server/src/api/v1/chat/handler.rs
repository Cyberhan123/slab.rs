use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::StreamExt;
use utoipa::OpenApi;

use crate::api::v1::chat::schema::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
    ChatModelOption, ChatModelSource, ChatReasoningEffort, ChatThinkingConfig, ChatThinkingType,
    ChatVerbosity,
};
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::models::{ChatCompletionOutput, ChatStreamChunk};
use crate::domain::services::ChatService;
use crate::error::ServerError;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions, list_chat_models),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        ChatModelOption,
        ChatModelSource,
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
