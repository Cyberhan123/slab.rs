//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].
//! - [`NoopNotifyAdapter`]: implements [`AgentNotifyPort`] as a no-op placeholder
//!   (fan-out to SSE/WebSocket is out of scope for P4-P6).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentNotifyPort, LlmPort, LlmResponse, ParsedToolCall, ThreadStatus, ToolSpec,
};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatStreamOptions, CloudChatParams,
    CommonChatParams, LocalChatParams,
};

// ── ServerLlmAdapter ─────────────────────────────────────────────────────────

/// Adapts the slab-server [`ModelState`] (and the chat pipeline behind it) into
/// a [`LlmPort`] that `AgentControl` can use.
///
/// Tool specs are forwarded through a text protocol because not every configured
/// backend exposes native function calling.
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
        tools: &[ToolSpec],
        config: &AgentConfig,
    ) -> Result<LlmResponse, AgentError> {
        let messages = messages_with_tool_protocol(messages, tools);
        let command = ChatCompletionCommand {
            id: None,
            model: model.to_owned(),
            messages,
            continue_generation: false,
            common: CommonChatParams {
                max_tokens: Some(config.max_tokens),
                temperature: Some(config.temperature),
                top_p: None,
                top_k: None,
                min_p: None,
                presence_penalty: None,
                repetition_penalty: None,
                n: 1,
                stream: false,
                stop: vec![],
                stream_options: ChatStreamOptions::default(),
            },
            local: LocalChatParams { gbnf: None, structured_output: None },
            cloud: CloudChatParams {
                reasoning_effort: None,
                verbosity: None,
                structured_output: None,
            },
        };

        let svc = crate::domain::services::ChatService::new((*self.state).clone());
        let output = svc.create_chat_completion(command).await.map_err(|e| {
            warn!(error = %e, "ServerLlmAdapter: chat completion failed");
            AgentError::Llm(e.to_string())
        })?;

        match output {
            ChatCompletionOutput::Json(result) => {
                let choice =
                    result.choices.into_iter().next().ok_or_else(|| {
                        AgentError::Llm("LLM returned an empty choices array".into())
                    })?;

                let mut tool_calls: Vec<ParsedToolCall> = choice
                    .message
                    .tool_calls
                    .into_iter()
                    .map(|tc| ParsedToolCall {
                        id: tc
                            .id
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| Uuid::new_v4().to_string()),
                        name: tc.function.name,
                        arguments: tc.function.arguments,
                    })
                    .collect();

                let content = match choice.message.content {
                    ConversationMessageContent::Text(t) if !t.is_empty() => Some(t),
                    _ => None,
                };
                let content = if tool_calls.is_empty() {
                    if let Some(text) = content.as_deref() {
                        tool_calls = parse_text_tool_calls(text);
                    }
                    if tool_calls.is_empty() { content } else { None }
                } else {
                    content
                };

                Ok(LlmResponse { content, tool_calls, finish_reason: choice.finish_reason })
            }
            ChatCompletionOutput::Stream(_) => Err(AgentError::Llm(
                "ServerLlmAdapter received an unexpected streaming response".into(),
            )),
        }
    }
}

fn messages_with_tool_protocol(
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
) -> Vec<ConversationMessage> {
    if tools.is_empty() {
        return messages.to_vec();
    }

    let mut messages = messages.to_vec();
    let insert_at = messages
        .iter()
        .take_while(|message| matches!(message.role.as_str(), "system" | "developer"))
        .count();
    messages.insert(
        insert_at,
        ConversationMessage {
            role: "system".to_owned(),
            content: ConversationMessageContent::Text(tool_protocol_prompt(tools)),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
    );
    messages
}

fn tool_protocol_prompt(tools: &[ToolSpec]) -> String {
    let tools_json = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters_schema,
            })
        })
        .collect::<Vec<_>>();
    let tools_text = serde_json::to_string_pretty(&tools_json).unwrap_or_else(|_| "[]".to_owned());

    format!(
        "You can call Slab tools when needed. To call tools, reply with only valid JSON and no markdown: {{\"tool_calls\":[{{\"name\":\"tool_name\",\"arguments\":{{}}}}]}}. After tool results appear in the conversation, answer normally. Available tools:\n{tools_text}"
    )
}

fn parse_text_tool_calls(content: &str) -> Vec<ParsedToolCall> {
    let Some(value) = parse_tool_json(content) else {
        return Vec::new();
    };

    if let Some(calls) = value.get("tool_calls").and_then(Value::as_array) {
        return calls.iter().filter_map(parse_tool_call_value).collect();
    }

    if let Some(call) = value.get("tool_call") {
        return parse_tool_call_value(call).into_iter().collect();
    }

    parse_tool_call_value(&value).into_iter().collect()
}

fn parse_tool_json(content: &str) -> Option<Value> {
    let trimmed = strip_json_fence(content.trim());
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Some(value);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<Value>(&trimmed[start..=end]).ok()
}

fn strip_json_fence(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("```") else {
        return content;
    };
    let after_header = rest.find('\n').map(|index| &rest[index + 1..]).unwrap_or(rest);
    after_header.strip_suffix("```").map(str::trim).unwrap_or(after_header.trim())
}

fn parse_tool_call_value(value: &Value) -> Option<ParsedToolCall> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/function/name").and_then(Value::as_str))?
        .trim();
    if name.is_empty() {
        return None;
    }

    let arguments = value
        .get("arguments")
        .or_else(|| value.pointer("/function/arguments"))
        .map(tool_arguments_to_string)
        .unwrap_or_else(|| "{}".to_owned());

    Some(ParsedToolCall {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        name: name.to_owned(),
        arguments,
    })
}

fn tool_arguments_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "{}".to_owned()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_protocol_response() {
        let calls = parse_text_tool_calls(
            r#"{"tool_calls":[{"name":"echo","arguments":{"message":"hello"}}]}"#,
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].arguments, r#"{"message":"hello"}"#);
    }

    #[test]
    fn ignores_plain_json_without_tool_fields() {
        let calls = parse_text_tool_calls(r#"{"answer":"hello"}"#);

        assert!(calls.is_empty());
    }
}
