//! OpenAI-compatible chat-completion routes.
//!
//! Delegates to the `ggml.llama` backend in slab-core.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use tracing::{debug, info};

use crate::error::ServerError;
use crate::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage,
};
use crate::state::AppState;

/// Register chat-completion routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/chat/completions", post(chat_completions))
}

/// OpenAI chat completions (`POST /v1/chat/completions`).
///
/// Forwards the final user message to the `ggml.llama` backend and returns the
/// generated text wrapped in an OpenAI-compatible JSON envelope.
///
/// The `stream` field is accepted for API compatibility; streaming via SSE is
/// tracked for a future iteration.
#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    tag = "chat",
    request_body = ChatCompletionRequest,
    responses(
        (status = 200, description = "Completion generated",  body = ChatCompletionResponse),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn chat_completions(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, ServerError> {
    // Use the last user-role message as the prompt.
    let prompt = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .ok_or_else(|| ServerError::BadRequest("no user message found".into()))?;

    debug!(model = %req.model, prompt_len = prompt.len(), "chat completion request");

    let result_bytes = slab_core::api::backend("ggml.llama")
        .op("inference")
        .input(slab_core::Payload::Text(std::sync::Arc::from(
            prompt.as_str(),
        )))
        .options(slab_core::Payload::Json(serde_json::json!({
            "max_tokens":  req.max_tokens.unwrap_or(512),
            "temperature": req.temperature.unwrap_or(0.7),
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    let generated = String::from_utf8(result_bytes.to_vec())
        .map_err(|e| ServerError::Internal(format!("backend returned invalid UTF-8: {e}")))?;

    info!(model = %req.model, output_len = generated.len(), "chat completion done");

    let resp = ChatCompletionResponse {
        id:      format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object:  "chat.completion".into(),
        created: chrono::Utc::now().timestamp(),
        model:   req.model,
        choices: vec![ChatChoice {
            index:         0,
            message:       ChatMessage { role: "assistant".into(), content: generated },
            finish_reason: "stop".into(),
        }],
    };

    Ok(Json(resp))
}
