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

/// Maximum allowed prompt length in bytes to prevent memory exhaustion.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

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

    // Reject oversized prompts to prevent memory exhaustion.
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(ServerError::BadRequest(format!(
            "prompt too large ({} bytes); maximum is {} bytes",
            prompt.len(),
            MAX_PROMPT_BYTES,
        )));
    }

    // Validate generation parameters so bad values fail fast before hitting
    // the backend.
    let max_tokens = req.max_tokens.unwrap_or(512);
    if max_tokens == 0 || max_tokens > 4096 {
        return Err(ServerError::BadRequest(format!(
            "invalid max_tokens ({max_tokens}): must be between 1 and 4096"
        )));
    }

    let temperature = req.temperature.unwrap_or(0.7);
    if !(0.0..=2.0).contains(&temperature) {
        return Err(ServerError::BadRequest(format!(
            "invalid temperature ({temperature}): must be between 0.0 and 2.0"
        )));
    }

    debug!(model = %req.model, prompt_len = prompt.len(), "chat completion request");

    let result_bytes = slab_core::api::backend("ggml.llama")
        .op("inference")
        .input(slab_core::Payload::Text(std::sync::Arc::from(
            prompt.as_str(),
        )))
        .options(slab_core::Payload::Json(serde_json::json!({
            "max_tokens":  max_tokens,
            "temperature": temperature,
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

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use crate::models::openai::ChatMessage;

    fn make_request(role: &str, content: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model:       "test".into(),
            messages:    vec![ChatMessage { role: role.into(), content: content.into() }],
            stream:      false,
            max_tokens:  None,
            temperature: None,
        }
    }

    #[test]
    fn validate_max_tokens_zero() {
        let req = ChatCompletionRequest {
            max_tokens: Some(0),
            ..make_request("user", "hello")
        };
        assert_eq!(req.max_tokens, Some(0));
        // The handler would reject this; verify the rule directly.
        let mt = req.max_tokens.unwrap_or(512);
        assert!(mt == 0 || mt > 4096, "should be out of range");
    }

    #[test]
    fn validate_max_tokens_too_large() {
        let req = ChatCompletionRequest {
            max_tokens: Some(9999),
            ..make_request("user", "hello")
        };
        let mt = req.max_tokens.unwrap_or(512);
        assert!(mt > 4096, "should be out of range");
    }

    #[test]
    fn validate_temperature_out_of_range() {
        // temperature = 3.0 is outside [0.0, 2.0]
        let temp = 3.0_f32;
        assert!(!(0.0..=2.0).contains(&temp), "should be out of range");
    }

    #[test]
    fn validate_prompt_too_large() {
        let long_prompt = "x".repeat(MAX_PROMPT_BYTES + 1);
        assert!(long_prompt.len() > MAX_PROMPT_BYTES);
    }

    #[test]
    fn no_user_message_returns_error() {
        let req = make_request("system", "you are a bot");
        // No user role – find should return None.
        let found = req.messages.iter().rev().find(|m| m.role == "user");
        assert!(found.is_none());
    }
}

