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
    ChatModelOption, ChatModelSource,
};
use crate::api::validation::ValidatedJson;
use crate::context::AppState;
use crate::domain::services::{
    to_chat_completion_command, to_chat_completion_response, to_chat_model_option_response,
    ChatCompletionOutput, ChatStreamChunk,
};
use crate::error::ServerError;
use crate::services::chat::ChatService;

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions, list_chat_models),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        OpenAiMessage,
        ChatChoice,
        ChatModelOption,
        ChatModelSource
    ))
)]
pub struct ChatApi;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/chat/models", get(list_chat_models))
}

#[utoipa::path(
    get,
    path = "/v1/chat/models",
    tag = "chat",
    responses(
        (status = 200, description = "Selectable chat models (local + cloud providers)", body = [ChatModelOption]),
        (status = 500, description = "Backend error"),
    )
)]
async fn list_chat_models(
    State(service): State<ChatService>,
) -> Result<Json<Vec<ChatModelOption>>, ServerError> {
    let models = service
        .list_chat_models()
        .await?
        .into_iter()
        .map(to_chat_model_option_response)
        .collect();
    Ok(Json(models))
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
    match service
        .create_chat_completion(to_chat_completion_command(req))
        .await?
    {
        ChatCompletionOutput::Json(response) => {
            Ok(Json(to_chat_completion_response(response)).into_response())
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
