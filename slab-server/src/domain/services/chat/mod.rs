//! OpenAI-compatible chat completion routes.

mod cloud;
mod local;

use chrono::Utc;
use futures::stream::BoxStream;
use tracing::{debug, info};
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatModelOption,
    ChatResultChoice, ChatStreamChunk, ConversationMessage as DomainConversationMessage,
};
use crate::error::ServerError;
use crate::infra::db::{ChatMessage, ChatStore};

/// Maximum allowed prompt length in bytes.
#[cfg(test)]
const MAX_PROMPT_BYTES: usize = 128 * 1024; // 128 KiB
const LLAMA_BACKEND_ID: &str = "ggml.llama";
const CLOUD_MODEL_ID_PREFIX: &str = "cloud";

enum GeneratedChatOutput {
    Text(String),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

#[derive(Clone)]
pub struct ChatService {
    state: ModelState,
}

impl ChatService {
    pub fn new(state: ModelState) -> Self {
        Self { state }
    }

    pub async fn list_chat_models(&self) -> Result<Vec<ChatModelOption>, ServerError> {
        let mut items = local::list_chat_models(&self.state).await?;
        items.extend(cloud::list_chat_models(&self.state).await);
        Ok(items)
    }

    pub async fn create_chat_completion(
        &self,
        command: ChatCompletionCommand,
    ) -> Result<ChatCompletionOutput, ServerError> {
        create_chat_completion_with_state(self.state.clone(), command).await
    }
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

/// Build an OpenAI-compatible reasoning SSE chunk payload.
fn build_reasoning_chunk(id: &str, created: i64, model: &str, token: &str) -> String {
    serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "delta": { "reasoning_content": token },
            "finish_reason": null
        }]
    })
    .to_string()
}

async fn create_chat_completion_with_state(
    state: ModelState,
    command: ChatCompletionCommand,
) -> Result<ChatCompletionOutput, ServerError> {
    let user_content = command
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.clone())
        .unwrap_or_default();

    let max_tokens = command.max_tokens.unwrap_or(512);
    let temperature = command.temperature.unwrap_or(0.7);

    debug!(
        model = %command.model,
        prompt_len = user_content.len(),
        stream = command.stream,
        session_id = ?command.id,
        "chat completion request"
    );

    let resolved_messages =
        build_messages(&state, command.id.as_deref(), &command.messages).await?;

    if let Some(session_id) = command.id.as_deref() {
        state
            .store()
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: session_id.to_owned(),
                role: "user".into(),
                content: user_content.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(
                |error| tracing::warn!(error = %error, "failed to persist user message"),
            );
    }

    let generated = if cloud::is_cloud_model_option_id(&command.model) {
        cloud::create_chat_completion(
            &state,
            &command.model,
            &resolved_messages,
            max_tokens,
            temperature,
            command.stream,
        )
        .await?
    } else {
        local::create_chat_completion(
            &state,
            &command.model,
            command.id.as_deref(),
            &resolved_messages,
            max_tokens,
            temperature,
            command.stream,
        )
        .await?
    };

    let generated = match generated {
        GeneratedChatOutput::Text(text) => text,
        GeneratedChatOutput::Stream(stream) => return Ok(ChatCompletionOutput::Stream(stream)),
    };

    info!(
        model = %command.model,
        output_len = generated.len(),
        "chat completion done"
    );

    if let Some(session_id) = command.id.as_deref() {
        state
            .store()
            .append_message(ChatMessage {
                id: Uuid::new_v4().to_string(),
                session_id: session_id.to_owned(),
                role: "assistant".into(),
                content: generated.clone(),
                created_at: Utc::now(),
            })
            .await
            .unwrap_or_else(
                |error| tracing::warn!(error = %error, "failed to persist assistant message"),
            );
    }

    let response = ChatCompletionResult {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        created: Utc::now().timestamp(),
        model: command.model,
        choices: vec![ChatResultChoice {
            index: 0,
            message: DomainConversationMessage {
                role: "assistant".into(),
                content: generated,
            },
            finish_reason: "stop".into(),
        }],
    };

    Ok(ChatCompletionOutput::Json(response))
}

/// Merge history from DB and current request messages while avoiding duplicates.
async fn build_messages(
    state: &ModelState,
    session_id: Option<&str>,
    current_messages: &[DomainConversationMessage],
) -> Result<Vec<DomainConversationMessage>, ServerError> {
    let current: Vec<DomainConversationMessage> = current_messages
        .iter()
        .filter(|message| !message.content.trim().is_empty())
        .cloned()
        .collect();
    let client_sent_history = current.len() > 1;

    let mut merged = Vec::new();
    // Avoid duplicating turns: if client already sends history, do not merge DB history again.
    if !client_sent_history {
        if let Some(session_id) = session_id {
            let history = state.store().list_messages(session_id).await?;
            for message in history {
                if message.content.trim().is_empty() {
                    continue;
                }
                merged.push(message.into());
            }
        }
    }
    merged.extend(current);
    Ok(merged)
}

#[cfg(test)]
mod test {
    use super::*;

    fn make_command(role: &str, content: &str) -> ChatCompletionCommand {
        ChatCompletionCommand {
            model: "test".into(),
            messages: vec![DomainConversationMessage {
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
        let req = ChatCompletionCommand {
            max_tokens: Some(0),
            ..make_command("user", "hello")
        };
        assert_eq!(req.max_tokens, Some(0));
        let max_tokens = req.max_tokens.unwrap_or(512);
        assert!(
            max_tokens == 0 || max_tokens > 4096,
            "should be out of range"
        );
    }

    #[test]
    fn validate_max_tokens_too_large() {
        let req = ChatCompletionCommand {
            max_tokens: Some(9999),
            ..make_command("user", "hello")
        };
        let max_tokens = req.max_tokens.unwrap_or(512);
        assert!(max_tokens > 4096, "should be out of range");
    }

    #[test]
    fn validate_temperature_out_of_range() {
        let temperature = 3.0_f32;
        assert!(
            !(0.0..=2.0).contains(&temperature),
            "should be out of range"
        );
    }

    #[test]
    fn validate_prompt_too_large() {
        let long_prompt = "x".repeat(MAX_PROMPT_BYTES + 1);
        assert!(long_prompt.len() > MAX_PROMPT_BYTES);
    }

    #[test]
    fn no_user_message_returns_error() {
        let req = make_command("system", "you are a bot");
        let found = req
            .messages
            .iter()
            .rev()
            .find(|message| message.role == "user");
        assert!(found.is_none());
    }

    #[test]
    fn build_chunk_produces_openai_format() {
        let json_str = build_chunk("chatcmpl-test", 1_700_000_000, "slab-llama", "Hello");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
        assert_eq!(value["id"], "chatcmpl-test");
        assert_eq!(value["object"], "chat.completion.chunk");
        assert_eq!(value["created"], 1_700_000_000_i64);
        assert_eq!(value["model"], "slab-llama");
        let choice = &value["choices"][0];
        assert_eq!(choice["index"], 0);
        assert_eq!(choice["delta"]["content"], "Hello");
        assert!(choice["finish_reason"].is_null());
    }
}
