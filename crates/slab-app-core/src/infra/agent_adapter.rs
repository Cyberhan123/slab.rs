//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].
//! - [`NoopNotifyAdapter`]: implements [`AgentNotifyPort`] as a no-op placeholder
//!   (fan-out to SSE/WebSocket is out of scope for P4-P6).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::{Map, Value};
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentNotifyPort, LlmPort, LlmResponse, LlmStreamObserver, ParsedToolCall, ThreadStatus,
    ToolSpec,
};
use slab_proto::openai::{FunctionTool, FunctionToolCall, FunctionToolType};
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
/// Tool specs are forwarded as Responses-style function tools and rendered by
/// the selected provider/template layer.
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
        let command =
            chat_command_from_agent_config(model, messages.to_vec(), tools, config, false);

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
        let command = chat_command_from_agent_config(
            model,
            messages.to_vec(),
            tools,
            config,
            tools.is_empty(),
        );

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
    tools: &[ToolSpec],
    config: &AgentConfig,
    stream: bool,
) -> ChatCompletionCommand {
    ChatCompletionCommand {
        id: None,
        model: model.to_owned(),
        messages,
        tools: response_function_tools_from_agent_tools(tools),
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

fn response_function_tools_from_agent_tools(tools: &[ToolSpec]) -> Vec<FunctionTool> {
    tools
        .iter()
        .map(|tool| {
            let parameters = match &tool.parameters_schema {
                Value::Object(map) => {
                    Some(map.iter().map(|(key, value)| (key.clone(), value.clone())).collect())
                }
                _ => None,
            };
            let mut function_tool = FunctionTool::new(
                FunctionToolType::Function,
                tool.name.clone(),
                parameters,
                Some(true),
            );
            if !tool.description.trim().is_empty() {
                function_tool.description = Some(Some(tool.description.clone()));
            }
            function_tool
        })
        .collect()
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
            tool_calls = parse_rendered_tool_calls(text);
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
            observer.on_reasoning_delta(&reasoning_delta).await?;
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

    let mut tool_calls = parse_rendered_tool_calls(&content);
    observer.on_reasoning_done(&reasoning).await?;
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

fn parse_rendered_tool_calls(content: &str) -> Vec<ParsedToolCall> {
    if let Some(value) = parse_tool_json(content) {
        let calls = parse_responses_tool_calls(&value);
        if !calls.is_empty() {
            return calls;
        }
    }

    parse_qwen_tool_calls(content)
}

fn parse_tool_json(content: &str) -> Option<Value> {
    let trimmed = strip_json_fence(content.trim());
    serde_json::from_str::<Value>(trimmed).ok()
}

fn strip_json_fence(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("```") else {
        return content;
    };
    let after_header = rest.find('\n').map(|index| &rest[index + 1..]).unwrap_or(rest);
    after_header.strip_suffix("```").map(str::trim).unwrap_or(after_header.trim())
}

fn parse_responses_tool_calls(value: &Value) -> Vec<ParsedToolCall> {
    if let Some(items) = value.get("output").and_then(Value::as_array) {
        return items.iter().filter_map(parse_responses_function_call).collect();
    }

    parse_responses_function_call(value).into_iter().collect()
}

fn parse_responses_function_call(value: &Value) -> Option<ParsedToolCall> {
    let call: FunctionToolCall = serde_json::from_value(value.clone()).ok()?;
    let name = call.name.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    let id = if call.call_id.trim().is_empty() {
        call.id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    } else {
        call.call_id
    };

    Some(ParsedToolCall { id, name, arguments: normalize_arguments(call.arguments) })
}

fn parse_qwen_tool_calls(content: &str) -> Vec<ParsedToolCall> {
    let mut rest = strip_reasoning_prefix(content.trim());
    let mut calls = Vec::new();

    while let Some(after_open) = rest.strip_prefix("<tool_call>") {
        let Some(close_start) = after_open.find("</tool_call>") else {
            return Vec::new();
        };
        let block = &after_open[..close_start];
        let Some(call) = parse_qwen_tool_call_block(block.trim()) else {
            return Vec::new();
        };
        calls.push(call);
        rest = after_open[close_start + "</tool_call>".len()..].trim_start();
    }

    if rest.trim().is_empty() { calls } else { Vec::new() }
}

fn strip_reasoning_prefix(content: &str) -> &str {
    let Some(close_start) = content.find("</think>") else {
        return content.trim();
    };
    let after_reasoning = content[close_start + "</think>".len()..].trim_start();
    if after_reasoning.starts_with("<tool_call>") || after_reasoning.starts_with('{') {
        after_reasoning
    } else {
        content.trim()
    }
}

fn parse_qwen_tool_call_block(block: &str) -> Option<ParsedToolCall> {
    let function_start = block.strip_prefix("<function=")?;
    let name_end = function_start.find('>')?;
    let name = function_start[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    let function_body = function_start[name_end + 1..].trim();
    let function_body = function_body.strip_suffix("</function>")?.trim();
    let arguments = parse_qwen_parameters(function_body)?;

    Some(ParsedToolCall {
        id: Uuid::new_v4().to_string(),
        name: name.to_owned(),
        arguments: serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_owned()),
    })
}

fn parse_qwen_parameters(mut input: &str) -> Option<Value> {
    let mut arguments = Map::new();
    input = input.trim();
    while !input.is_empty() {
        let parameter_start = input.strip_prefix("<parameter=")?;
        let name_end = parameter_start.find('>')?;
        let name = parameter_start[..name_end].trim();
        if name.is_empty() {
            return None;
        }
        let value_start = &parameter_start[name_end + 1..];
        let value_end = value_start.find("</parameter>")?;
        let raw_value = value_start[..value_end].trim();
        let value = serde_json::from_str::<Value>(raw_value)
            .unwrap_or_else(|_| Value::String(raw_value.to_owned()));
        arguments.insert(name.to_owned(), value);
        input = value_start[value_end + "</parameter>".len()..].trim();
    }
    Some(Value::Object(arguments))
}

fn normalize_arguments(arguments: String) -> String {
    serde_json::from_str::<Value>(&arguments)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| if arguments.trim().is_empty() { "{}".to_owned() } else { arguments })
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
    fn parses_responses_function_call_output() {
        let calls = parse_rendered_tool_calls(
            r#"{"output":[{"type":"function_call","call_id":"call-1","name":"echo","arguments":"{\"message\":\"hello\"}"}]}"#,
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call-1");
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].arguments, r#"{"message":"hello"}"#);
    }

    #[test]
    fn ignores_plain_json_without_tool_fields() {
        let calls = parse_rendered_tool_calls(r#"{"answer":"hello"}"#);

        assert!(calls.is_empty());
    }

    #[test]
    fn ignores_embedded_json_tool_calls_in_plain_text() {
        let calls = parse_rendered_tool_calls(
            r#"Please run this: {"output":[{"type":"function_call","call_id":"call-1","name":"echo","arguments":"{}"}]}"#,
        );

        assert!(calls.is_empty());
    }

    #[test]
    fn parses_qwen_template_tool_call_output() {
        let calls = parse_rendered_tool_calls(
            "<tool_call>\n<function=echo>\n<parameter=message>\nhello\n</parameter>\n</function>\n</tool_call>",
        );

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "echo");
        assert_eq!(calls[0].arguments, r#"{"message":"hello"}"#);
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

        let command = chat_command_from_agent_config("mock", Vec::new(), &[], &config, true);

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
        assert!(command.tools.is_empty());
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

    #[tokio::test]
    async fn forwards_chat_stream_reasoning_events() {
        use futures::StreamExt as _;

        struct RecordingObserver {
            text_delta: Vec<String>,
            reasoning_delta: Vec<String>,
            reasoning_done: Vec<String>,
        }

        #[async_trait]
        impl LlmStreamObserver for RecordingObserver {
            async fn on_text_delta(&mut self, delta: &str) -> Result<(), AgentError> {
                self.text_delta.push(delta.to_owned());
                Ok(())
            }

            async fn on_reasoning_delta(&mut self, delta: &str) -> Result<(), AgentError> {
                self.reasoning_delta.push(delta.to_owned());
                Ok(())
            }

            async fn on_reasoning_done(&mut self, text: &str) -> Result<(), AgentError> {
                self.reasoning_done.push(text.to_owned());
                Ok(())
            }
        }

        let stream = futures::stream::iter([
            ChatStreamChunk::Data(
                r#"{"choices":[{"delta":{"reasoning_content":"plan "}}]}"#.to_owned(),
            ),
            ChatStreamChunk::Data(
                r#"{"choices":[{"delta":{"reasoning_content":"done","content":"answer"}}]}"#
                    .to_owned(),
            ),
            ChatStreamChunk::Data(
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_owned(),
            ),
        ])
        .boxed();
        let mut observer = RecordingObserver {
            text_delta: Vec::new(),
            reasoning_delta: Vec::new(),
            reasoning_done: Vec::new(),
        };

        let response =
            llm_response_from_chat_stream(stream, &mut observer).await.expect("stream response");

        assert_eq!(observer.text_delta, ["answer"]);
        assert_eq!(observer.reasoning_delta, ["plan ", "done"]);
        assert_eq!(observer.reasoning_done, ["plan done"]);
        assert_eq!(
            response.content.as_deref(),
            Some("<think status=\"done\">\n\nplan done\n\n</think>\n\nanswer")
        );
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
