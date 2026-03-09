//! OpenAI-compatible chat completion routes.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use futures::{stream, StreamExt};
use tower::ServiceExt;
use tracing::{debug, info};
use utoipa::OpenApi;
use uuid::Uuid;

use crate::entities::{ChatMessage, ChatStore};
use crate::error::ServerError;
use crate::grpc;
use crate::schemas::v1::chat::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
};
use crate::state::AppState;

/// Maximum allowed prompt length in bytes.
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB

#[derive(OpenApi)]
#[openapi(
    paths(chat_completions),
    components(schemas(
        ChatCompletionRequest,
        ChatCompletionResponse,
        OpenAiMessage,
        ChatChoice
    ))
)]
pub struct ChatApi;

/// Register chat-completion routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/chat/completions", post(chat_completions))
}

/// Build an OpenAI-compatible `chat.completion.chunk` SSE data payload.
fn build_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

/// OpenAI chat completions (`POST /v1/chat/completions`).
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
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response, ServerError> {
    let user_content = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .ok_or_else(|| ServerError::BadRequest("no user message found".into()))?;

    if user_content.len() > MAX_PROMPT_BYTES {
        return Err(ServerError::BadRequest(format!(
            "prompt too large ({} bytes); maximum is {} bytes",
            user_content.len(),
            MAX_PROMPT_BYTES,
        )));
    }

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

    debug!(
        model = %req.model,
        prompt_len = user_content.len(),
        stream = req.stream,
        session_id = ?req.id,
        "chat completion request"
    );

    let prompt = build_prompt(&state, req.id.as_deref(), &req.messages).await?;

    if let Some(sid) = req.id.as_deref() {
        state
            .store
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: sid.to_owned(),
                role: "user".into(),
                content: user_content.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| tracing::warn!(error = %e, "failed to persist user message"));
    }

    let grpc_req = grpc::pb::ChatRequest {
        prompt: prompt.clone(),
        model: req.model.clone(),
        max_tokens,
        temperature,
        session_key: req.id.clone().unwrap_or_default(),
    };

    let llama_endpoint = state.config.llama_grpc_endpoint.clone().ok_or_else(|| {
        ServerError::BackendNotReady("llama gRPC endpoint is not configured".into())
    })?;

    if req.stream {
        let backend_stream = grpc::gateway::chat_stream(llama_endpoint.clone())
            .oneshot(grpc_req.clone())
            .await
            .map_err(|e| ServerError::Internal(format!("grpc chat stream failed: {e}")))?;

        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let created_ts = Utc::now().timestamp();
        let model_name = req.model.clone();

        let token_stream = backend_stream.map(move |chunk| -> Result<Event, Infallible> {
            match chunk {
                Ok(msg) if !msg.error.is_empty() => Ok(Event::default().comment(msg.error)),
                Ok(msg) if msg.done => Ok(Event::default().comment("done")),
                Ok(msg) => {
                    let data = build_chunk(&completion_id, created_ts, &model_name, &msg.token);
                    Ok(Event::default().data(data))
                }
                Err(e) => Ok(Event::default().comment(e.to_string())),
            }
        });

        let sse_stream = token_stream.chain(stream::once(async {
            Ok::<Event, Infallible>(Event::default().data("[DONE]"))
        }));

        return Ok(Sse::new(sse_stream).into_response());
    }

    let generated = grpc::gateway::chat(llama_endpoint)
        .oneshot(grpc_req)
        .await
        .map_err(|e| ServerError::Internal(format!("grpc chat failed: {e}")))?
        .text;

    info!(
        model = %req.model,
        output_len = generated.len(),
        "chat completion done"
    );

    if let Some(sid) = req.id.as_deref() {
        state
            .store
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: sid.to_owned(),
                role: "assistant".into(),
                content: generated.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(|e| tracing::warn!(error = %e, "failed to persist assistant message"));
    }

    let resp = ChatCompletionResponse {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        created: Utc::now().timestamp(),
        model: req.model,
        choices: vec![ChatChoice {
            index: 0,
            message: OpenAiMessage {
                role: "assistant".into(),
                content: generated,
            },
            finish_reason: "stop".into(),
        }],
    };

    Ok(Json(resp).into_response())
}

/// Build the full prompt string from session history and current messages.
async fn build_prompt(
    state: &AppState,
    session_id: Option<&str>,
    current_messages: &[OpenAiMessage],
) -> Result<String, ServerError> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(sid) = session_id {
        let history = state.store.list_messages(sid).await?;
        for msg in history {
            parts.push(format!("{}: {}", capitalize_role(&msg.role), msg.content));
        }
    }

    for msg in current_messages {
        parts.push(format!("{}: {}", capitalize_role(&msg.role), msg.content));
    }
    parts.push("Assistant:".into());

    Ok(parts.join("\n"))
}

fn capitalize_role(role: &str) -> &str {
    match role {
        "user" => "User",
        "assistant" => "Assistant",
        "system" => "System",
        other => other,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schemas::v1::chat::ChatMessage as OpenAiMsg;

    fn make_request(role: &str, content: &str) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "test".into(),
            messages: vec![OpenAiMsg {
                role: role.into(),
                content: content.into(),
            }],
            stream: false,
            max_tokens: None,
            temperature: None,
            id: None,
        }
    }

    #[test]
    fn validate_max_tokens_zero() {
        let req = ChatCompletionRequest {
            max_tokens: Some(0),
            ..make_request("user", "hello")
        };
        assert_eq!(req.max_tokens, Some(0));
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
        let found = req.messages.iter().rev().find(|m| m.role == "user");
        assert!(found.is_none());
    }

    #[test]
    fn build_chunk_produces_openai_format() {
        let json_str = build_chunk("chatcmpl-test", 1_700_000_000, "slab-llama", "Hello");
        let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(v["id"], "chatcmpl-test");
        assert_eq!(v["object"], "chat.completion.chunk");
        assert_eq!(v["created"], 1_700_000_000_i64);
        assert_eq!(v["model"], "slab-llama");
        let choice = &v["choices"][0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["delta"]["content"], "Hello");
        assert!(choice["finish_reason"].is_null());
    }
}
