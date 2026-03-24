//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].
//! - [`NoopNotifyAdapter`]: implements [`AgentNotifyPort`] as a no-op placeholder
//!   (fan-out to SSE/WebSocket is out of scope for P4-P6).

use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{AgentNotifyPort, LlmPort, LlmResponse, ParsedToolCall, ThreadStatus, ToolSpec};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{ChatCompletionCommand, ChatCompletionOutput, ChatStreamOptions};

// ── ServerLlmAdapter ─────────────────────────────────────────────────────────

/// Adapts the slab-server [`ModelState`] (and the chat pipeline behind it) into
/// a [`LlmPort`] that `AgentControl` can use.
///
/// Tool specs are currently not forwarded to the model because the local
/// llama.cpp backend does not expose a function-calling API.  Cloud providers
/// that support function calling can be wired in a future iteration.
#[derive(Clone)]
pub struct ServerLlmAdapter {
    state: Arc<ModelState>,
}

impl ServerLlmAdapter {
    pub fn new(state: Arc<ModelState>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl LlmPort for ServerLlmAdapter {
    async fn chat_completion(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        config: &AgentConfig,
    ) -> Result<LlmResponse, AgentError> {
        let command = ChatCompletionCommand {
            id: None,
            model: model.to_owned(),
            messages: messages.to_vec(),
            continue_generation: false,
            max_tokens: Some(config.max_tokens),
            temperature: Some(config.temperature),
            top_p: None,
            n: 1,
            stop: vec![],
            grammar: None,
            grammar_json: false,
            structured_output: None,
            reasoning_effort: None,
            verbosity: None,
            stream: false,
            stream_options: ChatStreamOptions::default(),
        };

        let svc = crate::domain::services::ChatService::new((*self.state).clone());
        let output = svc.create_chat_completion(command).await.map_err(|e| {
            warn!(error = %e, "ServerLlmAdapter: chat completion failed");
            AgentError::Llm(e.to_string())
        })?;

        match output {
            ChatCompletionOutput::Json(result) => {
                let choice = result.choices.into_iter().next().ok_or_else(|| {
                    AgentError::Llm("LLM returned an empty choices array".into())
                })?;

                let tool_calls: Vec<ParsedToolCall> = choice
                    .message
                    .tool_calls
                    .into_iter()
                    .map(|tc| ParsedToolCall {
                        id: tc.id.filter(|s| !s.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string()),
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    })
                    .collect();

                let content = match choice.message.content {
                    ConversationMessageContent::Text(t) if !t.is_empty() => Some(t),
                    _ => None,
                };

                Ok(LlmResponse {
                    content,
                    tool_calls,
                    finish_reason: choice.finish_reason,
                })
            }
            ChatCompletionOutput::Stream(_) => Err(AgentError::Llm(
                "ServerLlmAdapter received an unexpected streaming response".into(),
            )),
        }
    }
}

// ── NoopNotifyAdapter ─────────────────────────────────────────────────────────

/// A no-op [`AgentNotifyPort`] that discards all status-change notifications.
///
/// Replace with a real fan-out adapter (SSE, WebSocket) when the frontend
/// needs real-time agent status streaming.
pub struct NoopNotifyAdapter;

#[async_trait]
impl AgentNotifyPort for NoopNotifyAdapter {
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus) {
        tracing::debug!(thread_id, ?status, "agent status change (noop notify)");
    }
}
