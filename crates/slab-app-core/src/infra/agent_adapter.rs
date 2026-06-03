//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].
//! - [`NoopNotifyAdapter`]: implements [`AgentNotifyPort`] as a no-op placeholder
//!   (fan-out to SSE/WebSocket is out of scope for P4-P6).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentNotifyPort, LlmPort, LlmResponse, LlmStreamObserver, ParsedToolCall, ThreadStatus,
    ToolSpec,
};
use slab_proto::openai::{FunctionTool, FunctionToolType};
use slab_types::{ConversationMessage, ConversationMessageContent};
use tracing::warn;
use uuid::Uuid;

use crate::context::ModelState;
use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionOutput, ChatCompletionResult, ChatStreamChunk,
    ChatStreamOptions, CloudChatParams, CommonChatParams, LocalChatParams,
    assistant_message_from_parts,
};
use crate::infra::agent_stream_parser::{
    AgentStreamAssembler, AgentStreamDelta, parse_rendered_tool_call_output,
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
        let command = chat_command_from_agent_config(model, messages.to_vec(), tools, config, true);

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
            let parsed = parse_rendered_tool_call_output(text);
            tool_calls = parsed.tool_calls;
            if !tool_calls.is_empty() {
                return Ok(LlmResponse {
                    content: parsed.content,
                    content_already_streamed: false,
                    tool_calls,
                    finish_reason: choice.finish_reason,
                });
            }
        }
        content
    } else {
        content
    };

    Ok(LlmResponse {
        content,
        content_already_streamed: false,
        tool_calls,
        finish_reason: choice.finish_reason,
    })
}

async fn llm_response_from_chat_stream(
    mut stream: futures::stream::BoxStream<'static, ChatStreamChunk>,
    observer: &mut dyn LlmStreamObserver,
) -> Result<LlmResponse, AgentError> {
    let mut assembler = AgentStreamAssembler::default();

    while let Some(chunk) = stream.next().await {
        let ChatStreamChunk::Data(data) = chunk;
        for delta in assembler.ingest_data(&data)? {
            match delta {
                AgentStreamDelta::Text(text) => observer.on_text_delta(&text).await?,
                AgentStreamDelta::Reasoning(reasoning) => {
                    observer.on_reasoning_delta(&reasoning).await?;
                }
            }
        }
    }

    let completion = assembler.finish();
    if let Some(delta) = completion.unstreamed_text_delta.as_deref() {
        observer.on_text_delta(delta).await?;
    }
    observer.on_reasoning_done(&completion.reasoning).await?;
    let content = if completion.tool_calls.is_empty() {
        response_content_from_stream_parts(&completion.content, &completion.reasoning)
    } else {
        response_content_from_stream_parts(&completion.content, &completion.reasoning)
    };

    Ok(LlmResponse {
        content,
        content_already_streamed: completion.content_already_streamed,
        tool_calls: completion.tool_calls,
        finish_reason: completion.finish_reason,
    })
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
}
