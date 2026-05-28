//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].
//! - [`NoopNotifyAdapter`]: implements [`AgentNotifyPort`] as a no-op placeholder
//!   (fan-out to SSE/WebSocket is out of scope for P4-P6).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Value, json};
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentNotifyPort, LlmPort, LlmResponse, LlmStreamObserver, ParsedToolCall, ThreadStatus,
    ToolSpec,
};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatStreamChunk,
    ChatStreamOptions, CloudChatParams, CommonChatParams, LocalChatParams,
    assistant_message_from_parts,
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
        let command = chat_command_from_agent_config(model, messages, config, false);

        let svc = crate::domain::services::ChatService::new((*self.state).clone());
        let output = svc.create_chat_completion(command).await.map_err(|e| {
            warn!(error = %e, "ServerLlmAdapter: chat completion failed");
            AgentError::Llm(e.to_string())
        })?;

        match output {
            ChatCompletionOutput::Json(result) => llm_response_from_chat_result(result),
            ChatCompletionOutput::Stream(_) => Err(AgentError::Llm(
                "ServerLlmAdapter received an unexpected streaming response".into(),
            )),
        }
    }

    async fn chat_completion_streaming(
        &self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        config: &AgentConfig,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        let messages = messages_with_tool_protocol(messages, tools);
        let command = chat_command_from_agent_config(model, messages, config, true);

        let svc = crate::domain::services::ChatService::new((*self.state).clone());
        let output = svc.create_chat_completion(command).await.map_err(|e| {
            warn!(error = %e, "ServerLlmAdapter: streaming chat completion failed");
            AgentError::Llm(e.to_string())
        })?;

        match output {
            ChatCompletionOutput::Json(result) => {
                let response = llm_response_from_chat_result(result)?;
                if response.tool_calls.is_empty()
                    && let Some(content) = response.content.as_deref()
                    && !content.is_empty()
                {
                    observer.on_text_delta(content).await?;
                }
                Ok(response)
            }
            ChatCompletionOutput::Stream(stream) => {
                llm_response_from_chat_stream(stream, observer).await
            }
        }
    }
}

fn chat_command_from_agent_config(
    model: &str,
    messages: Vec<ConversationMessage>,
    config: &AgentConfig,
    stream: bool,
) -> ChatCompletionCommand {
    ChatCompletionCommand {
        id: None,
        model: model.to_owned(),
        messages,
        continue_generation: false,
        common: CommonChatParams {
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            top_p: config.top_p,
            top_k: config.top_k,
            min_p: config.min_p,
            presence_penalty: config.presence_penalty,
            repetition_penalty: config.repetition_penalty,
            n: 1,
            stream,
            stop: vec![],
            stream_options: ChatStreamOptions::default(),
        },
        local: LocalChatParams { gbnf: None, structured_output: None },
        cloud: CloudChatParams {
            reasoning_effort: config.reasoning_effort,
            verbosity: config.verbosity,
            structured_output: None,
        },
    }
}

fn llm_response_from_chat_result(result: ChatCompletionResult) -> Result<LlmResponse, AgentError> {
    let choice = result
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| AgentError::Llm("LLM returned an empty choices array".into()))?;

    let mut tool_calls: Vec<ParsedToolCall> = choice
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

async fn llm_response_from_chat_stream(
    mut stream: futures::stream::BoxStream<'static, ChatStreamChunk>,
    observer: &mut dyn LlmStreamObserver,
) -> Result<LlmResponse, AgentError> {
    let mut content = String::new();
    let mut reasoning = String::new();
    let mut finish_reason = None;
    let mut visibility = StreamVisibilityGate::default();

    while let Some(chunk) = stream.next().await {
        let ChatStreamChunk::Data(data) = chunk;
        let Some(parsed) = parse_chat_stream_chunk(&data)? else {
            continue;
        };

        if let Some(reasoning_delta) = parsed.reasoning_delta {
            reasoning.push_str(&reasoning_delta);
        }
        if let Some(content_delta) = parsed.content_delta {
            content.push_str(&content_delta);
            if let Some(visible_delta) = visibility.ingest(&content_delta) {
                observer.on_text_delta(&visible_delta).await?;
            }
        }
        if parsed.finish_reason.is_some() {
            finish_reason = parsed.finish_reason;
        }
    }

    let mut tool_calls = parse_text_tool_calls(&content);
    let content = if tool_calls.is_empty() {
        response_content_from_stream_parts(&content, &reasoning)
    } else {
        None
    };

    Ok(LlmResponse { content, tool_calls: std::mem::take(&mut tool_calls), finish_reason })
}

fn response_content_from_stream_parts(content: &str, reasoning: &str) -> Option<String> {
    if content.is_empty() && reasoning.trim().is_empty() {
        return None;
    }

    Some(
        assistant_message_from_parts(content, (!reasoning.trim().is_empty()).then_some(reasoning))
            .rendered_text(),
    )
}

#[derive(Default)]
struct ParsedChatStreamChunk {
    content_delta: Option<String>,
    reasoning_delta: Option<String>,
    finish_reason: Option<String>,
}

fn parse_chat_stream_chunk(data: &str) -> Result<Option<ParsedChatStreamChunk>, AgentError> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(None);
    }

    let Ok(payload) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(None);
    };

    if let Some(message) = stream_error_message(&payload) {
        return Err(AgentError::Llm(message));
    }

    Ok(Some(ParsedChatStreamChunk {
        content_delta: collect_text_delta(&payload, "content"),
        reasoning_delta: collect_text_delta(&payload, "reasoning_content"),
        finish_reason: stream_finish_reason(&payload),
    }))
}

fn stream_error_message(payload: &Value) -> Option<String> {
    let error = payload.get("error")?;
    error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .map(str::to_owned)
        .or_else(|| Some("LLM stream returned an error".to_owned()))
}

fn collect_text_delta(payload: &Value, field: &str) -> Option<String> {
    let text = payload
        .get("choices")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|choice| {
            choice.get("delta").and_then(|delta| delta.get(field)).and_then(Value::as_str)
        })
        .filter(|value| !value.is_empty())
        .collect::<String>();
    if text.is_empty() { None } else { Some(text) }
}

fn stream_finish_reason(payload: &Value) -> Option<String> {
    payload
        .get("choices")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|choice| choice.get("finish_reason").and_then(Value::as_str))
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

#[derive(Default)]
struct StreamVisibilityGate {
    pending: String,
    streaming: bool,
}

impl StreamVisibilityGate {
    fn ingest(&mut self, delta: &str) -> Option<String> {
        if self.streaming {
            return Some(delta.to_owned());
        }

        self.pending.push_str(delta);
        if stream_prefix_is_plain_text(&self.pending) {
            self.streaming = true;
            Some(std::mem::take(&mut self.pending))
        } else {
            None
        }
    }
}

fn stream_prefix_is_plain_text(buffer: &str) -> bool {
    let trimmed = buffer.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
        return false;
    }

    let Some(rest) = trimmed.strip_prefix("```") else {
        return true;
    };
    let Some(newline) = rest.find('\n') else {
        return false;
    };
    let language = rest[..newline].trim();
    !language.is_empty() && !language.eq_ignore_ascii_case("json")
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

    #[test]
    fn agent_config_params_are_forwarded_to_chat_command() {
        let config = AgentConfig {
            model: "mock".into(),
            max_tokens: Some(4096),
            temperature: Some(0.2),
            top_p: Some(0.9),
            top_k: Some(40),
            min_p: Some(0.1),
            presence_penalty: Some(0.3),
            repetition_penalty: Some(1.05),
            reasoning_effort: Some(slab_types::chat::ChatReasoningEffort::Low),
            verbosity: Some(slab_types::chat::ChatVerbosity::Medium),
            ..AgentConfig::default()
        };

        let command = chat_command_from_agent_config("mock", Vec::new(), &config, true);

        assert_eq!(command.common.max_tokens, Some(4096));
        assert_eq!(command.common.temperature, Some(0.2));
        assert_eq!(command.common.top_p, Some(0.9));
        assert_eq!(command.common.top_k, Some(40));
        assert_eq!(command.common.min_p, Some(0.1));
        assert_eq!(command.common.presence_penalty, Some(0.3));
        assert_eq!(command.common.repetition_penalty, Some(1.05));
        assert_eq!(
            command.cloud.reasoning_effort,
            Some(slab_types::chat::ChatReasoningEffort::Low)
        );
        assert_eq!(command.cloud.verbosity, Some(slab_types::chat::ChatVerbosity::Medium));
        assert!(command.common.stream);
    }

    #[test]
    fn parses_chat_stream_content_and_finish_chunks() {
        let chunk = parse_chat_stream_chunk(
            r#"{"choices":[{"delta":{"content":"hel"},"finish_reason":null}]}"#,
        )
        .expect("valid chunk")
        .expect("parsed chunk");

        assert_eq!(chunk.content_delta.as_deref(), Some("hel"));
        assert_eq!(chunk.finish_reason, None);

        let finish =
            parse_chat_stream_chunk(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#)
                .expect("valid chunk")
                .expect("parsed chunk");

        assert_eq!(finish.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn visibility_gate_holds_tool_call_json_until_classified() {
        let mut gate = StreamVisibilityGate::default();

        assert_eq!(gate.ingest("{"), None);
        assert_eq!(gate.ingest(r#""tool_calls":["#), None);
    }

    #[test]
    fn visibility_gate_flushes_plain_text() {
        let mut gate = StreamVisibilityGate::default();

        assert_eq!(gate.ingest("hel").as_deref(), Some("hel"));
        assert_eq!(gate.ingest("lo").as_deref(), Some("lo"));
    }

    #[test]
    fn visibility_gate_flushes_non_json_code_fences_after_language() {
        let mut gate = StreamVisibilityGate::default();

        assert_eq!(gate.ingest("```"), None);
        assert_eq!(gate.ingest("rust\nfn main() {}").as_deref(), Some("```rust\nfn main() {}"));
    }
}
