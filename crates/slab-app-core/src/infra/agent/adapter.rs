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
    port::{LlmPort, LlmResponse, LlmStreamObserver, LlmUsage, ParsedToolCall, ToolSpec},
};
use slab_agent_tracing::{AgentTraceContext, record_json_from_context};
use slab_proto::openai::FunctionTool;
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
            if e2e_mode_enabled() {
                e2e_apply_slow_delay(messages).await;
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
            slab_agent::classify_llm_error(&e.to_string())
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
            if e2e_mode_enabled() {
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
            slab_agent::classify_llm_error(&e.to_string())
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
pub fn e2e_mode_enabled() -> bool {
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
    e2e_apply_slow_delay(messages).await;
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

    // Subagent delegation track: child turns are keyed by the fixed prefixes
    // the delegation machinery produces (objective / steering / completion
    // notification); parent turns by `subagent-e2e/…` markers the tests
    // control. Falls through to the legacy plan loop / echo behavior below.
    if let Some(response) = e2e_subagent_response(&prompt, has_tool_result_after_prompt, tools) {
        return response;
    }

    let normalized_prompt = prompt.to_ascii_lowercase();
    let wants_plan_loop = normalized_prompt.contains("tool loop")
        || normalized_prompt.contains("plan_update")
        || normalized_prompt.contains("plan update");

    if wants_plan_loop && !has_tool_result_after_prompt && e2e_tool_available(tools, "plan") {
        return LlmResponse {
            content: None,
            content_already_streamed: false,
            tool_calls: vec![ParsedToolCall {
                id: "e2e-plan".to_owned(),
                name: "plan".to_owned(),
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
            usage: None,
        };
    }

    let content = if wants_plan_loop && has_tool_result_after_prompt {
        "E2E loop complete after plan tool output.".to_owned()
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
        usage: None,
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

/// Pause the scripted response when the prompt asks for a slow turn, so
/// stop/interrupt/steering races become deterministic. The parent honors the
/// explicit `subagent-e2e/slow/<ms>` marker; child objective turns also honor
/// `slow=<ms>` embedded in the delegated task text (that text shows up
/// verbatim in the parent prompt too — the parent must not sleep).
#[cfg(any(test, debug_assertions))]
async fn e2e_apply_slow_delay(messages: &[ConversationMessage]) {
    let delay_ms = e2e_slow_ms(&e2e_latest_user_context(messages).0);
    if delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }
}

/// Deterministic response track for the background-subagent e2e suite.
/// Returns `None` when the prompt carries no subagent marker, letting the
/// legacy plan-loop / echo branches handle it.
#[cfg(any(test, debug_assertions))]
fn e2e_subagent_response(
    prompt: &str,
    has_tool_result_after_prompt: bool,
    tools: &[ToolSpec],
) -> Option<LlmResponse> {
    // Child steering turn — guidance the parent injected mid-flight.
    if prompt.starts_with("[steering from parent agent]") {
        let needle = e2e_parse_marker(prompt, "needle2").unwrap_or_else(|| "unmarked".to_owned());
        return Some(e2e_text_response(format!("SUBAGENT_STEERING_ACK {needle}")));
    }

    // Child objective turn — the delegation's fixed first user message
    // (`render_child_task`).
    if prompt.starts_with("Objective:") {
        let needle =
            e2e_parse_marker(prompt, "needle").unwrap_or_else(|| "generic-child-done".to_owned());
        return Some(e2e_text_response(format!("SUBAGENT_RESULT {needle}")));
    }

    // Parent turn resumed by the completion notification (`render_notification`).
    if prompt.starts_with("[subagent task finished] task_id=") {
        let task_id = e2e_parse_marker(prompt, "task_id").unwrap_or_else(|| "unknown".to_owned());
        return Some(e2e_text_response(format!(
            "E2E parent resumed after subagent task {task_id} finished."
        )));
    }

    let carries_marker = prompt.contains("subagent-e2e/delegate")
        || prompt.contains("subagent-e2e/steer")
        || prompt.contains("subagent-e2e/stop")
        || prompt.contains("subagent-e2e/shell-sleep/");

    // Parent follow-up once the delegated/steering tool result is in — close
    // the turn so the background work can proceed server-side.
    if carries_marker && has_tool_result_after_prompt {
        return Some(e2e_text_response("E2E delegated in the background; continuing.".to_owned()));
    }

    if !carries_marker || has_tool_result_after_prompt {
        // Only the plain slow marker below applies to unmarked prompts.
        return e2e_slow_marker_response(prompt);
    }

    if prompt.contains("subagent-e2e/delegate") && e2e_tool_available(tools, "delegate_subagent") {
        let task = e2e_parse_quoted(prompt, "task")
            .unwrap_or_else(|| "e2e delegated task needle=SUBAGENT_NEEDLE".to_owned());
        return Some(e2e_tool_call_response(
            "e2e-delegate",
            "delegate_subagent",
            serde_json::json!({ "task": task, "background": true }),
        ));
    }

    if prompt.contains("subagent-e2e/steer") && e2e_tool_available(tools, "subagent_message") {
        let task_id = e2e_parse_marker(prompt, "task_id").unwrap_or_else(|| "unknown".to_owned());
        let message = e2e_parse_quoted(prompt, "message")
            .unwrap_or_else(|| "e2e steering message".to_owned());
        return Some(e2e_tool_call_response(
            "e2e-steer",
            "subagent_message",
            serde_json::json!({ "task_id": task_id, "message": message }),
        ));
    }

    if prompt.contains("subagent-e2e/stop") && e2e_tool_available(tools, "subagent_stop") {
        let task_id = e2e_parse_marker(prompt, "task_id").unwrap_or_else(|| "unknown".to_owned());
        return Some(e2e_tool_call_response(
            "e2e-stop",
            "subagent_stop",
            serde_json::json!({ "task_id": task_id }),
        ));
    }

    if prompt.contains("subagent-e2e/shell-sleep/") && e2e_tool_available(tools, "shell") {
        let sleep_ms = e2e_parse_marker_ms(prompt, "subagent-e2e/shell-sleep/").unwrap_or(30_000);
        return Some(e2e_tool_call_response(
            "e2e-shell-sleep",
            "shell",
            serde_json::json!({ "command": format!("sleep {}", sleep_ms / 1000) }),
        ));
    }

    None
}

/// Plain delayed reply that keeps the turn busy (e.g. so Stop can be clicked
/// mid-flight). The delay itself is applied by [`e2e_apply_slow_delay`].
#[cfg(any(test, debug_assertions))]
fn e2e_slow_marker_response(prompt: &str) -> Option<LlmResponse> {
    let sleep_ms = e2e_parse_marker_ms(prompt, "subagent-e2e/slow/")?;
    Some(e2e_text_response(format!("E2E slow reply after {sleep_ms}ms.")))
}

#[cfg(any(test, debug_assertions))]
fn e2e_text_response(content: String) -> LlmResponse {
    LlmResponse {
        content: Some(content),
        content_already_streamed: false,
        tool_calls: Vec::new(),
        finish_reason: Some("stop".to_owned()),
        usage: None,
    }
}

#[cfg(any(test, debug_assertions))]
fn e2e_tool_call_response(id: &str, name: &str, arguments: serde_json::Value) -> LlmResponse {
    LlmResponse {
        content: None,
        content_already_streamed: false,
        tool_calls: vec![ParsedToolCall {
            id: id.to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_string(),
        }],
        finish_reason: Some("tool_calls".to_owned()),
        usage: None,
    }
}

/// Find `key=<TOKEN>` where `<TOKEN>` is the following run of non-whitespace.
#[cfg(any(test, debug_assertions))]
fn e2e_parse_marker(prompt: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=");
    prompt
        .find(&pattern)
        .map(|start| {
            prompt[start + pattern.len()..]
                .chars()
                .take_while(|c| !c.is_whitespace())
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
}

/// Find `key="..."` (plain double-quoted value; the marker grammar the e2e
/// prompts use needs no escape handling).
#[cfg(any(test, debug_assertions))]
fn e2e_parse_quoted(prompt: &str, key: &str) -> Option<String> {
    let pattern = format!("{key}=\"");
    let start = prompt.find(&pattern)? + pattern.len();
    let rest = &prompt[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    (!value.is_empty()).then(|| value.to_owned())
}

/// Parent-side explicit slow marker (`subagent-e2e/slow/<ms>`) and — for child
/// objective turns only — the `slow=<ms>` embedded in the delegated task text.
#[cfg(any(test, debug_assertions))]
fn e2e_slow_ms(prompt: &str) -> u64 {
    const MAX_SLOW_MS: u64 = 120_000;
    let requested = e2e_parse_marker_ms(prompt, "subagent-e2e/slow/")
        .or_else(|| {
            prompt.starts_with("Objective:").then(|| e2e_parse_marker_ms(prompt, "slow=")).flatten()
        })
        .unwrap_or(0);
    requested.min(MAX_SLOW_MS)
}

/// Parse the ASCII digits immediately following `prefix` as milliseconds.
#[cfg(any(test, debug_assertions))]
fn e2e_parse_marker_ms(prompt: &str, prefix: &str) -> Option<u64> {
    let start = prompt.find(prefix)? + prefix.len();
    let digits: String = prompt[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Derive a stable kv-cache session key for the agent path.
///
/// Prefers the thread id (stable across all turns of a conversation) and falls
/// back to the session id. The key lets the local-LLM runtime reuse its
/// managed-session snapshot between turns so only the new turn is prefilled.
fn agent_kv_session_key(trace_context: &AgentTraceContext) -> Option<String> {
    if let Some(thread_id) = trace_context.thread_id.as_ref()
        && !thread_id.trim().is_empty()
    {
        return Some(format!("agent:{thread_id}"));
    }
    let session = trace_context.session_id.trim();
    (!session.is_empty()).then(|| format!("agent:{session}"))
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
        local: LocalChatParams {
            gbnf: None,
            structured_output: config.structured_output.clone(),
            // Stable per-thread key so the local-LLM managed-session snapshot
            // cache engages and only the new turn is re-prefilled each turn
            // (incremental prefill). Kept separate from `id` (which stays
            // `None`) so the chat_messages history/persistence machinery is
            // not triggered for the agent path.
            session_key: agent_kv_session_key(trace_context),
            // The agent context hook already injects a reasoning-effort fragment,
            // so the local path must not inject its inline policy a second time.
            reasoning_guidance_in_context: true,
        },
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
            let mut function_tool = FunctionTool::new(tool.name.clone(), parameters, Some(true));
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
                    usage: result.usage.map(Into::into),
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
        usage: result.usage.map(Into::into),
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
            "usage": completion.usage,
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
        usage: completion.usage,
    })
}

impl From<crate::domain::models::TextGenerationUsage> for LlmUsage {
    fn from(value: crate::domain::models::TextGenerationUsage) -> Self {
        Self {
            prompt_tokens: value.prompt_tokens,
            completion_tokens: value.completion_tokens,
            total_tokens: value.total_tokens,
            estimated: value.estimated,
        }
    }
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
            "usage": response.usage,
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
    use crate::domain::models::ChatResultChoice;

    fn text_message(role: &str, content: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(content.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn plan_spec() -> ToolSpec {
        ToolSpec {
            name: "plan".to_owned(),
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
    fn e2e_llm_requests_plan_before_tool_result() {
        let response =
            e2e_llm_response(&[text_message("user", "please run the tool loop")], &[plan_spec()]);

        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "plan");
        assert!(response.tool_calls[0].arguments.contains("record plan"));
    }

    #[test]
    fn e2e_llm_finishes_after_tool_result() {
        let response = e2e_llm_response(
            &[
                text_message("user", "please run the tool loop"),
                text_message("tool", "{\"summary\":\"e2e assistant loop\"}"),
            ],
            &[plan_spec()],
        );

        assert_eq!(response.content.as_deref(), Some("E2E loop complete after plan tool output."));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason.as_deref(), Some("stop"));
    }

    fn subagent_spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_owned(),
            description: "subagent e2e tool".to_owned(),
            parameters_schema: serde_json::json!({ "type": "object" }),
        }
    }

    #[test]
    fn e2e_child_objective_turn_returns_needle() {
        let response = e2e_llm_response(
            &[text_message(
                "user",
                "Objective:\ncount needles needle=ALPHA_1 slow=20000\n\nConstraints:\n- Work only on this delegated task.",
            )],
            &[],
        );

        assert_eq!(response.content.as_deref(), Some("SUBAGENT_RESULT ALPHA_1"));
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn e2e_child_objective_turn_falls_back_without_needle() {
        let response = e2e_llm_response(
            &[text_message(
                "user",
                "Objective:\ndo the thing\n\nConstraints:\n- Work only on this delegated task.",
            )],
            &[],
        );

        assert_eq!(response.content.as_deref(), Some("SUBAGENT_RESULT generic-child-done"));
    }

    #[test]
    fn e2e_child_steering_turn_returns_needle2() {
        let response = e2e_llm_response(
            &[text_message("user", "[steering from parent agent]\nalso cover needle2=BETA_2")],
            &[],
        );

        assert_eq!(response.content.as_deref(), Some("SUBAGENT_STEERING_ACK BETA_2"));
    }

    #[test]
    fn e2e_parent_delegate_emits_structured_tool_call() {
        let response = e2e_llm_response(
            &[text_message(
                "user",
                "subagent-e2e/delegate task=\"count widgets needle=GAMMA_3 slow=20000\"",
            )],
            &[subagent_spec("delegate_subagent")],
        );

        assert_eq!(response.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(response.tool_calls.len(), 1);
        let call = &response.tool_calls[0];
        assert_eq!(call.name, "delegate_subagent");
        let arguments: serde_json::Value =
            serde_json::from_str(&call.arguments).expect("arguments are valid JSON");
        assert_eq!(arguments["task"], "count widgets needle=GAMMA_3 slow=20000");
        assert_eq!(arguments["background"], true);
    }

    #[test]
    fn e2e_parent_delegate_closing_text_after_tool_result() {
        let response = e2e_llm_response(
            &[
                text_message("user", "subagent-e2e/delegate task=\"count widgets needle=GAMMA_3\""),
                text_message("tool", "{\"background\":true,\"status\":\"running\"}"),
            ],
            &[subagent_spec("delegate_subagent")],
        );

        assert_eq!(
            response.content.as_deref(),
            Some("E2E delegated in the background; continuing.")
        );
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn e2e_parent_notification_turn_summarizes() {
        let response = e2e_llm_response(
            &[text_message(
                "user",
                "[subagent task finished] task_id=bg-x-1 status=completed\nTask: count widgets\nResult: SUBAGENT_RESULT GAMMA_3",
            )],
            &[],
        );

        let content = response.content.as_deref().expect("text response");
        assert!(content.contains("bg-x-1"), "content mentions the finished task id: {content}");
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn e2e_parent_steer_and_stop_tool_calls() {
        let steer = e2e_llm_response(
            &[text_message(
                "user",
                "subagent-e2e/steer task_id=bg-7 message=\"also cover needle2=DELTA_4\"",
            )],
            &[subagent_spec("subagent_message")],
        );
        assert_eq!(steer.tool_calls.len(), 1);
        assert_eq!(steer.tool_calls[0].name, "subagent_message");
        let arguments: serde_json::Value =
            serde_json::from_str(&steer.tool_calls[0].arguments).expect("steer arguments JSON");
        assert_eq!(arguments["task_id"], "bg-7");
        assert_eq!(arguments["message"], "also cover needle2=DELTA_4");

        let stop = e2e_llm_response(
            &[text_message("user", "subagent-e2e/stop task_id=bg-7")],
            &[subagent_spec("subagent_stop")],
        );
        assert_eq!(stop.tool_calls.len(), 1);
        assert_eq!(stop.tool_calls[0].name, "subagent_stop");
        let arguments: serde_json::Value =
            serde_json::from_str(&stop.tool_calls[0].arguments).expect("stop arguments JSON");
        assert_eq!(arguments["task_id"], "bg-7");
    }

    #[test]
    fn e2e_shell_sleep_tool_call_uses_posix_sleep() {
        let response = e2e_llm_response(
            &[text_message("user", "subagent-e2e/shell-sleep/30000 now")],
            &[subagent_spec("shell")],
        );

        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "shell");
        let arguments: serde_json::Value =
            serde_json::from_str(&response.tool_calls[0].arguments).expect("shell arguments JSON");
        assert_eq!(arguments["command"], "sleep 30");
    }

    #[test]
    fn e2e_slow_marker_returns_plain_delayed_text() {
        let response =
            e2e_llm_response(&[text_message("user", "hold subagent-e2e/slow/4000")], &[]);

        assert_eq!(response.content.as_deref(), Some("E2E slow reply after 4000ms."));
    }

    #[test]
    fn e2e_slow_ms_applies_to_parent_marker_and_child_objective_only() {
        // Parent honors the explicit slow marker.
        assert_eq!(e2e_slow_ms("please subagent-e2e/slow/5000"), 5_000);
        // Child objective turns honor the task-embedded `slow=` value…
        assert_eq!(e2e_slow_ms("Objective:\nprobe slow=20000\n\nConstraints:"), 20_000);
        // …but the parent must NOT sleep on the same text embedded in the
        // delegate args (the task shows up verbatim in the parent prompt).
        assert_eq!(e2e_slow_ms("subagent-e2e/delegate task=\"probe slow=90000\""), 0);
        // Clamped to a sane ceiling.
        assert_eq!(e2e_slow_ms("subagent-e2e/slow/9999999"), 120_000);
    }

    #[test]
    fn chat_result_usage_is_forwarded_to_agent_response() {
        let result = ChatCompletionResult {
            id: "chatcmpl-test".to_owned(),
            object: "chat.completion".to_owned(),
            created: 0,
            model: "mock".to_owned(),
            system_fingerprint: "test".to_owned(),
            choices: vec![ChatResultChoice {
                index: 0,
                message: text_message("assistant", "done"),
                finish_reason: Some("stop".to_owned()),
            }],
            usage: Some(crate::domain::models::TextGenerationUsage {
                prompt_tokens: 11,
                completion_tokens: 7,
                total_tokens: 18,
                prompt_tokens_details: Default::default(),
                estimated: false,
            }),
        };

        let response = llm_response_from_chat_result(result).expect("response");
        let usage = response.usage.expect("usage should be forwarded");

        assert_eq!(usage.prompt_tokens, 11);
        assert_eq!(usage.completion_tokens, 7);
        assert_eq!(usage.total_tokens, 18);
        assert!(!usage.estimated);
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
            r#"{"choices":[],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5,"estimated":true}}"#.to_owned(),
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
        assert_eq!(response.usage.expect("usage").total_tokens, 5);
    }
}
