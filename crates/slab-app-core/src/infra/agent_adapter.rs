//! Port adapters that connect `slab-agent`'s port traits to slab-server internals.
//!
//! - [`ServerLlmAdapter`]: implements [`LlmPort`] by delegating to the existing
//!   [`ChatService`][crate::domain::services::ChatService].

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::{
    AgentStreamAssembler, AgentStreamDelta, parse_rendered_tool_call_output,
    port::{LlmPort, LlmResponse, LlmStreamObserver, ParsedToolCall, ToolSpec},
};
use slab_agent_tracing::{AgentTraceContext, record_json_from_context};
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
        trace_context: &AgentTraceContext,
    ) -> Result<LlmResponse, AgentError> {
        #[cfg(any(test, debug_assertions))]
        {
            if e2e_llm_enabled() {
                let response = e2e_llm_response(messages, tools);
                record_llm_response(trace_context, "e2e_chat_response_normalized", &response);
                return Ok(response);
            }
        }

        let command = chat_command_from_agent_config(
            model,
            messages.to_vec(),
            tools,
            config,
            false,
            trace_context,
        );
        record_chat_command(trace_context, "chat_command_created", &command);

        let svc = crate::domain::services::ChatService::new((*self.state).clone());
        let output = svc.create_chat_completion(command).await.map_err(|e| {
            warn!(error = %e, "ServerLlmAdapter: chat completion failed");
            AgentError::Llm(e.to_string())
        })?;

        match output {
            ChatCompletionOutput::Json(result) => {
                let response = llm_response_from_chat_result(result)?;
                record_llm_response(trace_context, "chat_response_normalized", &response);
                Ok(response)
            }
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
        trace_context: &AgentTraceContext,
        observer: &mut dyn LlmStreamObserver,
    ) -> Result<LlmResponse, AgentError> {
        #[cfg(any(test, debug_assertions))]
        {
            if e2e_llm_enabled() {
                let response = e2e_llm_response_streaming(messages, tools, observer).await?;
                record_llm_response(trace_context, "e2e_chat_response_normalized", &response);
                return Ok(response);
            }
        }

        let command = chat_command_from_agent_config(
            model,
            messages.to_vec(),
            tools,
            config,
            true,
            trace_context,
        );
        record_chat_command(trace_context, "chat_command_created", &command);

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
                record_llm_response(trace_context, "chat_response_normalized", &response);
                Ok(response)
            }
            ChatCompletionOutput::Stream(stream) => {
                let response =
                    llm_response_from_chat_stream(stream, observer, trace_context).await?;
                record_llm_response(trace_context, "chat_stream_normalized", &response);
                Ok(response)
            }
        }
    }
}

#[cfg(any(test, debug_assertions))]
fn e2e_llm_enabled() -> bool {
    match std::env::var("SLAB_E2E_MODE") {
        Ok(value) => {
            let value = value.trim();
            value == "1" || value.eq_ignore_ascii_case("true")
        }
        Err(_) => false,
    }
}

#[cfg(any(test, debug_assertions))]
async fn e2e_llm_response_streaming(
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    observer: &mut dyn LlmStreamObserver,
) -> Result<LlmResponse, AgentError> {
    let mut response = e2e_llm_response(messages, tools);
    if response.tool_calls.is_empty()
        && let Some(content) = response.content.as_deref()
        && !content.is_empty()
    {
        observer.on_text_delta(content).await?;
        response.content_already_streamed = true;
    }
    Ok(response)
}

#[cfg(any(test, debug_assertions))]
fn e2e_llm_response(messages: &[ConversationMessage], tools: &[ToolSpec]) -> LlmResponse {
    let (prompt, has_tool_result_after_prompt) = e2e_latest_user_context(messages);
    let normalized_prompt = prompt.to_ascii_lowercase();
    let wants_plan_loop = normalized_prompt.contains("tool loop")
        || normalized_prompt.contains("plan_update")
        || normalized_prompt.contains("plan update");

    if wants_plan_loop && !has_tool_result_after_prompt && e2e_tool_available(tools, "plan_update")
    {
        return LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "e2e-plan-update".to_owned(),
                name: "plan_update".to_owned(),
                arguments: serde_json::json!({
                    "summary": "e2e assistant loop",
                    "items": [
                        { "step": "record plan", "status": "in_progress" },
                        { "step": "finish answer", "status": "pending" }
                    ]
                })
                .to_string(),
            }],
            finish_reason: Some("tool_calls".to_owned()),
        };
    }

    let content = if wants_plan_loop && has_tool_result_after_prompt {
        "E2E loop complete after plan_update tool output.".to_owned()
    } else if prompt.trim().is_empty() {
        "E2E assistant persisted reply.".to_owned()
    } else {
        format!("E2E assistant persisted reply: {prompt}")
    };

    LlmResponse {
        content: Some(content),
        content_already_streamed: false,
        tool_calls: Vec::new(),
        finish_reason: Some("stop".to_owned()),
    }
}

#[cfg(any(test, debug_assertions))]
fn e2e_latest_user_context(messages: &[ConversationMessage]) -> (String, bool) {
    let Some(latest_user_index) = messages.iter().rposition(|message| message.role == "user")
    else {
        return (String::new(), false);
    };

    let prompt = messages[latest_user_index].rendered_text();
    let has_tool_result_after_prompt =
        messages.iter().skip(latest_user_index + 1).any(|message| message.role == "tool");

    (prompt, has_tool_result_after_prompt)
}

#[cfg(any(test, debug_assertions))]
fn e2e_tool_available(tools: &[ToolSpec], tool_name: &str) -> bool {
    tools.iter().any(|tool| tool.name == tool_name)
}

fn chat_command_from_agent_config(
    model: &str,
    messages: Vec<ConversationMessage>,
    tools: &[ToolSpec],
    config: &AgentConfig,
    stream: bool,
    trace_context: &AgentTraceContext,
) -> ChatCompletionCommand {
    ChatCompletionCommand {
        id: None,
        model: model.to_owned(),
        messages,
        tools: response_function_tools_from_agent_tools(tools),
        agent_trace: Some(trace_context.clone()),
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
        local: LocalChatParams { gbnf: None, structured_output: config.structured_output.clone() },
        cloud: CloudChatParams {
            reasoning_effort: config.reasoning_effort,
            verbosity: config.verbosity,
            structured_output: config.structured_output.clone(),
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
    trace_context: &AgentTraceContext,
) -> Result<LlmResponse, AgentError> {
    let mut assembler = AgentStreamAssembler::default();

    while let Some(chunk) = stream.next().await {
        record_json_from_context(
            trace_context,
            "slab-app-core",
            "chat_stream_chunk",
            serde_json::json!({ "data": &chunk }),
        );
        for delta in assembler.ingest_data(&chunk)? {
            match delta {
                AgentStreamDelta::Text(text) => observer.on_text_delta(&text).await?,
                AgentStreamDelta::Reasoning(reasoning) => {
                    observer.on_reasoning_delta(&reasoning).await?;
                }
            }
        }
    }

    let completion = assembler.finish();
    record_json_from_context(
        trace_context,
        "slab-app-core",
        "chat_stream_assembled",
        serde_json::json!({
            "content": completion.content,
            "reasoning": completion.reasoning,
            "content_already_streamed": completion.content_already_streamed,
            "tool_calls": parsed_tool_calls_payload(&completion.tool_calls),
            "finish_reason": completion.finish_reason,
        }),
    );
    if let Some(delta) = completion.unstreamed_text_delta.as_deref() {
        observer.on_text_delta(delta).await?;
    }
    observer.on_reasoning_done(&completion.reasoning).await?;
    let content = response_content_from_stream_parts(&completion.content, &completion.reasoning);

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

fn record_chat_command(
    trace_context: &AgentTraceContext,
    event: &'static str,
    command: &ChatCompletionCommand,
) {
    record_json_from_context(
        trace_context,
        "slab-app-core",
        event,
        serde_json::json!({
            "id": command.id,
            "model": command.model,
            "messages": command.messages,
            "tools": command.tools,
            "continue_generation": command.continue_generation,
            "common": {
                "max_tokens": command.common.max_tokens,
                "temperature": command.common.temperature,
                "top_p": command.common.top_p,
                "top_k": command.common.top_k,
                "min_p": command.common.min_p,
                "presence_penalty": command.common.presence_penalty,
                "repetition_penalty": command.common.repetition_penalty,
                "n": command.common.n,
                "stream": command.common.stream,
                "stop": command.common.stop,
                "stream_options": {
                    "include_usage": command.common.stream_options.include_usage,
                },
            },
            "local": {
                "gbnf": command.local.gbnf,
                "structured_output": command.local.structured_output,
            },
            "cloud": {
                "reasoning_effort": command.cloud.reasoning_effort,
                "verbosity": command.cloud.verbosity,
                "structured_output": command.cloud.structured_output,
            },
        }),
    );
}

fn record_llm_response(
    trace_context: &AgentTraceContext,
    event: &'static str,
    response: &LlmResponse,
) {
    record_json_from_context(
        trace_context,
        "slab-app-core",
        event,
        serde_json::json!({
            "content": response.content,
            "content_already_streamed": response.content_already_streamed,
            "finish_reason": response.finish_reason,
            "tool_calls": parsed_tool_calls_payload(&response.tool_calls),
        }),
    );
}

fn parsed_tool_calls_payload(tool_calls: &[ParsedToolCall]) -> Vec<Value> {
    tool_calls
        .iter()
        .map(|tool_call| {
            serde_json::json!({
                "id": tool_call.id,
                "name": tool_call.name,
                "arguments": tool_call.arguments,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_message(role: &str, content: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(content.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn plan_update_spec() -> ToolSpec {
        ToolSpec {
            name: "plan_update".to_owned(),
            description: "record a plan".to_owned(),
            parameters_schema: serde_json::json!({ "type": "object" }),
        }
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
            structured_output: Some(slab_types::chat::StructuredOutput::JsonObject),
            ..AgentConfig::default()
        };

        let trace_context = AgentTraceContext::new("session");
        let command =
            chat_command_from_agent_config("mock", Vec::new(), &[], &config, true, &trace_context);

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
        assert_eq!(
            command.local.structured_output,
            Some(slab_types::chat::StructuredOutput::JsonObject)
        );
        assert_eq!(
            command.cloud.structured_output,
            Some(slab_types::chat::StructuredOutput::JsonObject)
        );
        assert!(command.common.stream);
        assert!(command.tools.is_empty());
    }

    #[test]
    fn e2e_llm_requests_plan_update_before_tool_result() {
        let response = e2e_llm_response(
            &[text_message("user", "please run the tool loop")],
            &[plan_update_spec()],
        );

        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "plan_update");
        assert!(response.tool_calls[0].arguments.contains("record plan"));
    }

    #[test]
    fn e2e_llm_finishes_after_tool_result() {
        let response = e2e_llm_response(
            &[
                text_message("user", "please run the tool loop"),
                text_message("tool", "{\"summary\":\"e2e assistant loop\"}"),
            ],
            &[plan_update_spec()],
        );

        assert_eq!(
            response.content.as_deref(),
            Some("E2E loop complete after plan_update tool output.")
        );
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
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
            r#"{"choices":[{"delta":{"reasoning_content":"plan "}}]}"#.to_owned(),
            r#"{"choices":[{"delta":{"reasoning_content":"done","content":"answer"}}]}"#.to_owned(),
            r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#.to_owned(),
        ])
        .boxed();
        let mut observer = RecordingObserver {
            text_delta: Vec::new(),
            reasoning_delta: Vec::new(),
            reasoning_done: Vec::new(),
        };

        let trace_context = AgentTraceContext::new("test-session");
        let response = llm_response_from_chat_stream(stream, &mut observer, &trace_context)
            .await
            .expect("stream response");

        assert_eq!(observer.text_delta, ["answer"]);
        assert_eq!(observer.reasoning_delta, ["plan ", "done"]);
        assert_eq!(observer.reasoning_done, ["plan done"]);
        assert_eq!(
            response.content.as_deref(),
            Some("<think status=\"done\">\n\nplan done\n\n</think>\n\nanswer")
        );
    }
}
