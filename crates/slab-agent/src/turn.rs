//! Single-turn execution logic (private to the crate).

use async_trait::async_trait;
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_agent_tracing::{AgentTraceContext, AgentTraceSink, record_json};
use slab_types::{
    ConversationMessage, ConversationMessageContent, ConversationToolCall, ConversationToolFunction,
};

use crate::{
    config::{AgentConfig, AgentToolChoice},
    error::AgentError,
    event::{AgentArtifactKind, AgentArtifactRef, AgentEventKind, AgentMetrics},
    hook::{AgentHookRegistry, HookEvent, dispatch_registered_hooks},
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalPort, LlmPort, LlmStreamObserver, ParsedToolCall,
        ThreadMessageRecord, ToolSpec, TurnEvent, TurnStateRecord,
    },
    repetition_guard::ToolCallSignature,
    risk::ToolRiskAnalyzer,
    tool::{AgentThreadContext, ToolRouter},
    tool_validation::{InvalidToolCall, validate_tool_calls},
    turn_tool_call::handle_tool_calls,
    turn_tool_record::record_failed_tool_call,
};

/// Execute a single LLM turn.
///
/// Returns `true` if another turn is needed (i.e. the model emitted tool
/// calls), or `false` when the model produced a final answer.
pub(crate) struct TurnExecutionContext<'a> {
    pub thread_id: &'a str,
    pub session_id: &'a str,
    pub turn_index: u32,
    pub depth: u32,
    pub config: &'a AgentConfig,
    pub llm: &'a dyn LlmPort,
    pub tools: &'a ToolRouter,
    pub store: &'a dyn AgentStorePort,
    pub notify: &'a dyn AgentNotifyPort,
    pub approval: &'a dyn ApprovalPort,
    pub hooks: &'a AgentHookRegistry,
    pub risk: &'a dyn ToolRiskAnalyzer,
    pub trace: &'a dyn AgentTraceSink,
    pub trace_context: AgentTraceContext,
    pub cancellation: &'a CancellationToken,
    pub thread_context: &'a AgentThreadContext,
    pub consumed_tokens: u32,
}

pub(crate) enum TurnOutcome {
    Final { token_usage: u32 },
    BudgetExceeded { token_usage: u32 },
    ToolCalls { invalid_tool_calls: usize, signatures: Vec<ToolCallSignature>, token_usage: u32 },
}

pub(crate) async fn execute_turn(
    context: TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
) -> Result<TurnOutcome, AgentError> {
    let turn_started_at = std::time::Instant::now();
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    let tool_specs = allowed_tool_specs(&context)?;
    let llm_start_effects = dispatch_registered_hooks(
        context.hooks,
        &HookEvent::OnLlmStart {
            thread_id: context.thread_id.to_owned(),
            session_id: context.session_id.to_owned(),
            turn_index: context.turn_index,
            messages: messages.clone(),
            tools: tool_specs.clone(),
        },
    )
    .await;
    insert_injected_messages(messages, llm_start_effects.injected_messages);
    append_observations(messages, llm_start_effects.observations);
    persist_turn_state(
        &context,
        "running",
        Some(messages.as_slice()),
        Some(&tool_specs),
        None,
        None,
        None,
    )
    .await;

    debug!(thread_id = context.thread_id, turn_index = context.turn_index, "executing turn");
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_started",
        serde_json::json!({
            "thread_id": context.thread_id,
            "turn_index": context.turn_index,
            "depth": context.depth,
            "message_count": messages.len(),
        }),
    );
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "agent_llm_request",
        serde_json::json!({
            "model": context.config.model,
            "messages": messages,
            "tools": tool_specs_trace_payload(&tool_specs),
            "config": context.config,
        }),
    );
    if let Some(structured_output) = context.config.structured_output.as_ref() {
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "structured_output_requested",
            serde_json::json!({ "structured_output": structured_output }),
        );
    }

    let mut stream_observer = TurnTextDeltaObserver {
        thread_id: context.thread_id,
        turn_index: context.turn_index,
        notify: context.notify,
    };
    let response_result = tokio::select! {
        response = context.llm.chat_completion_streaming(
            &context.config.model,
            messages,
            &tool_specs,
            context.config,
            &context.trace_context,
            &mut stream_observer,
        ) => response,
        _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
    };
    let response = match response_result {
        Ok(response) => response,
        Err(error) => {
            persist_turn_state(
                &context,
                "failed",
                Some(messages.as_slice()),
                Some(&tool_specs),
                None,
                Some(&error.to_string()),
                Some(Utc::now().to_rfc3339()),
            )
            .await;
            return Err(error);
        }
    };
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "llm_response_normalized",
        serde_json::json!({
            "content": &response.content,
            "content_already_streamed": response.content_already_streamed,
            "finish_reason": &response.finish_reason,
            "tool_calls": parsed_tool_calls_trace_payload(&response.tool_calls),
            "usage": response.usage,
        }),
    );
    persist_turn_state(
        &context,
        "llm_completed",
        Some(messages.as_slice()),
        Some(&tool_specs),
        Some(&response),
        None,
        None,
    )
    .await;
    let llm_end_effects = dispatch_registered_hooks(
        context.hooks,
        &HookEvent::OnLlmEnd {
            thread_id: context.thread_id.to_owned(),
            session_id: context.session_id.to_owned(),
            turn_index: context.turn_index,
            messages: messages.clone(),
            response: response.clone(),
        },
    )
    .await;
    insert_injected_messages(messages, llm_end_effects.injected_messages);
    append_observations(messages, llm_end_effects.observations);

    let token_usage = response.usage.as_ref().map(|usage| usage.total_tokens).unwrap_or_default();
    if token_budget_would_be_exhausted(
        context.config.token_budget,
        context.consumed_tokens,
        token_usage,
    ) {
        persist_turn_state(
            &context,
            "budget_exhausted",
            Some(messages.as_slice()),
            Some(&tool_specs),
            Some(&response),
            None,
            Some(Utc::now().to_rfc3339()),
        )
        .await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_token_budget_exhausted",
            serde_json::json!({
                "token_usage": token_usage,
                "consumed_tokens": context.consumed_tokens,
                "token_budget": context.config.token_budget,
                "has_tool_calls": !response.tool_calls.is_empty(),
            }),
        );
        return Ok(TurnOutcome::BudgetExceeded { token_usage });
    }

    if response.tool_calls.is_empty() {
        if let Err(error) = reject_missing_required_tool_call(&context) {
            persist_turn_state(
                &context,
                "failed",
                Some(messages.as_slice()),
                Some(&tool_specs),
                Some(&response),
                Some(&error.to_string()),
                Some(Utc::now().to_rfc3339()),
            )
            .await;
            return Err(error);
        }
        persist_final_answer(
            &context,
            messages,
            response.content.unwrap_or_default(),
            collect_turn_artifact_refs(messages),
            None,
        )
        .await;
        emit_turn_metrics(&context, turn_started_at, true).await;
        persist_turn_state(
            &context,
            "completed",
            Some(messages.as_slice()),
            Some(&tool_specs),
            None,
            None,
            Some(Utc::now().to_rfc3339()),
        )
        .await;
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "turn_completed",
            serde_json::json!({ "more_turns": false }),
        );
        return Ok(TurnOutcome::Final { token_usage });
    }

    let validation = validate_tool_calls(
        &context.config.tool_choice,
        &context.config.allowed_tools,
        &tool_specs,
        &response.tool_calls,
    );
    emit_unstreamed_tool_text(
        &context,
        response.content.as_deref(),
        response.content_already_streamed,
    )
    .await;
    persist_assistant_tool_request(&context, messages, &response).await;
    if !validation.invalid.is_empty() {
        record_invalid_tool_calls(&context, &validation.invalid, messages).await?;
    }
    if !validation.valid.is_empty() {
        let task_completion = handle_tool_calls(&context, &validation.valid, messages).await?;
        if let Some(completion) = task_completion {
            // 双轨 2: the deterministic `task.complete` gate passed; emit the
            // summary as the final answer and end the run (alongside the
            // existing `tool_calls.is_empty()` Final path).
            persist_final_answer(
                &context,
                messages,
                completion.summary,
                completion.artifact_refs,
                Some("completed".to_owned()),
            )
            .await;
            emit_turn_metrics(&context, turn_started_at, true).await;
            persist_turn_state(
                &context,
                "completed",
                Some(messages.as_slice()),
                Some(&tool_specs),
                None,
                None,
                Some(Utc::now().to_rfc3339()),
            )
            .await;
            record_json(
                context.trace,
                &context.trace_context,
                "slab-agent",
                "turn_completed",
                serde_json::json!({ "more_turns": false, "task_complete": true }),
            );
            return Ok(TurnOutcome::Final { token_usage });
        }
    }

    emit_turn_metrics(&context, turn_started_at, true).await;
    persist_turn_state(
        &context,
        "tool_calls_completed",
        Some(messages.as_slice()),
        Some(&tool_specs),
        None,
        None,
        Some(Utc::now().to_rfc3339()),
    )
    .await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "turn_completed",
        serde_json::json!({ "more_turns": true }),
    );
    Ok(TurnOutcome::ToolCalls {
        invalid_tool_calls: validation.invalid.len(),
        signatures: validation.valid.iter().map(ToolCallSignature::new).collect(),
        token_usage,
    })
}

fn token_budget_would_be_exhausted(
    token_budget: Option<u32>,
    consumed_tokens: u32,
    token_usage: u32,
) -> bool {
    token_budget
        .is_some_and(|budget| budget > 0 && consumed_tokens.saturating_add(token_usage) >= budget)
}

/// External tools that require provider/network reachability and are removed
/// from the agent's tool list in offline mode (INFRA-07). Local filesystem,
/// shell, plan, verify, and a2u surface tools remain available offline.
fn is_external_tool_name(name: &str) -> bool {
    matches!(name, "web_search" | "mcp_call" | "mcp_list_tools") || name.starts_with("mcp__")
}

fn allowed_tool_specs(context: &TurnExecutionContext<'_>) -> Result<Vec<ToolSpec>, AgentError> {
    let mut specs = context.tools.tool_specs();
    if !context.config.allowed_tools.is_empty() {
        specs.retain(|tool| context.config.allowed_tools.contains(&tool.name));
    }
    if context.thread_context.offline {
        // INFRA-07: offline mode narrows the toolset to local-only tools,
        // dropping anything that needs external network/provider reachability.
        specs.retain(|tool| !is_external_tool_name(&tool.name));
    }

    match &context.config.tool_choice {
        AgentToolChoice::Auto => Ok(specs),
        AgentToolChoice::None => Ok(Vec::new()),
        AgentToolChoice::Required => {
            if specs.is_empty() {
                Err(AgentError::Internal(
                    "tool_choice required but no tools are available".to_owned(),
                ))
            } else {
                Ok(specs)
            }
        }
        AgentToolChoice::Tool { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(AgentError::Internal(
                    "tool_choice tool name must not be blank".to_owned(),
                ));
            }
            let Some(spec) = specs.into_iter().find(|tool| tool.name == name) else {
                return Err(AgentError::Internal(format!(
                    "tool_choice tool is not available or allowed: {name}"
                )));
            };
            Ok(vec![spec])
        }
    }
}

fn reject_missing_required_tool_call(context: &TurnExecutionContext<'_>) -> Result<(), AgentError> {
    match &context.config.tool_choice {
        AgentToolChoice::Required => Err(AgentError::Internal(
            "tool_choice required but the model returned no tool calls".to_owned(),
        )),
        AgentToolChoice::Tool { name } => Err(AgentError::Internal(format!(
            "tool_choice requires tool '{name}' but the model returned no tool calls"
        ))),
        AgentToolChoice::Auto | AgentToolChoice::None => Ok(()),
    }
}

async fn record_invalid_tool_calls(
    context: &TurnExecutionContext<'_>,
    invalid: &[InvalidToolCall],
    messages: &mut Vec<ConversationMessage>,
) -> Result<(), AgentError> {
    let created_at = Utc::now().to_rfc3339();
    for invalid_call in invalid {
        let call_id = Uuid::new_v4().to_string();
        record_json(
            context.trace,
            &context.trace_context,
            "slab-agent",
            "invalid_tool_call",
            serde_json::json!({
                "item_id": invalid_call.tool_call.id,
                "call_id": call_id,
                "tool_name": invalid_call.tool_call.name,
                "arguments": invalid_call.tool_call.arguments,
                "reason": invalid_call.reason,
            }),
        );
        context
            .notify
            .on_turn_event(
                context.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(context.turn_index),
                    event: AgentEventKind::ResponseToolCallValidationFailed {
                        item_id: invalid_call.tool_call.id.clone(),
                        call_id: call_id.clone(),
                        tool_name: invalid_call.tool_call.name.clone(),
                        reason: invalid_call.reason.clone(),
                    },
                },
            )
            .await;
        record_failed_tool_call(
            context,
            &call_id,
            &invalid_call.tool_call,
            format!("invalid tool call: {}", invalid_call.reason),
            &created_at,
            messages,
        )
        .await?;
    }
    Ok(())
}

async fn persist_final_answer(
    context: &TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
    content: String,
    artifact_refs: Vec<AgentArtifactRef>,
    reason: Option<String>,
) {
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseOutputTextDone {
                    item_id: assistant_item_id(context.turn_index),
                    output_index: 0,
                    content_index: 0,
                    text: content.clone(),
                    artifact_refs,
                    reason,
                },
            },
        )
        .await;

    let message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(content),
        name: None,
        tool_call_id: None,
        tool_calls: vec![],
    };
    persist_thread_message(context.store, context.thread_id, context.turn_index, &message).await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "assistant_message_persisted",
        serde_json::json!({
            "turn_index": context.turn_index,
            "message": message,
        }),
    );
    messages.push(message);
}

fn collect_turn_artifact_refs(messages: &[ConversationMessage]) -> Vec<AgentArtifactRef> {
    let mut refs = messages
        .iter()
        .rev()
        .take_while(|message| message.role == "tool")
        .filter_map(tool_message_artifact_ref)
        .collect::<Vec<_>>();
    refs.reverse();
    refs
}

fn tool_message_artifact_ref(message: &ConversationMessage) -> Option<AgentArtifactRef> {
    let raw = message.content.rendered_text();
    let value = serde_json::from_str::<serde_json::Value>(raw.trim()).ok()?;
    let surface = value.get("surface").and_then(serde_json::Value::as_str)?;
    match surface {
        "workspace" => string_field(&value, "revealPath")
            .or_else(|| string_field(&value, "path"))
            .and_then(|path| normalize_workspace_artifact_path(&path))
            .map(|path| AgentArtifactRef { path, kind: AgentArtifactKind::File }),
        "review" => string_field(&value, "path")
            .and_then(|path| normalize_workspace_artifact_path(&path))
            .map(|path| AgentArtifactRef { path, kind: AgentArtifactKind::Diff }),
        "image" => string_field(&value, "path")
            .and_then(|path| normalize_workspace_artifact_path(&path))
            .map(|path| AgentArtifactRef { path, kind: AgentArtifactKind::Image }),
        _ => None,
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn normalize_workspace_artifact_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || is_absolute_or_drive_path(trimmed) {
        return None;
    }

    let normalized = trimmed.replace('\\', "/");
    let parts =
        normalized.split('/').filter(|part| !part.is_empty() && *part != ".").collect::<Vec<_>>();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }

    Some(parts.join("/"))
}

fn is_absolute_or_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || (bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':'))
}

async fn emit_unstreamed_tool_text(
    context: &TurnExecutionContext<'_>,
    content: Option<&str>,
    content_already_streamed: bool,
) {
    let Some(text) = content else {
        return;
    };
    if content_already_streamed || text.is_empty() {
        return;
    }

    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseOutputTextDelta {
                    item_id: assistant_item_id(context.turn_index),
                    output_index: 0,
                    content_index: 0,
                    delta: text.to_owned(),
                },
            },
        )
        .await;
}

async fn persist_assistant_tool_request(
    context: &TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
    response: &crate::port::LlmResponse,
) {
    let assistant_tool_calls: Vec<ConversationToolCall> = response
        .tool_calls
        .iter()
        .map(|tool_call| ConversationToolCall {
            id: Some(tool_call.id.clone()),
            r#type: "function".to_owned(),
            function: ConversationToolFunction {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
            },
        })
        .collect();

    let assistant_message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(response.content.clone().unwrap_or_default()),
        name: None,
        tool_call_id: None,
        tool_calls: assistant_tool_calls,
    };
    persist_thread_message(
        context.store,
        context.thread_id,
        context.turn_index,
        &assistant_message,
    )
    .await;
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "assistant_message_persisted",
        serde_json::json!({
            "turn_index": context.turn_index,
            "message": assistant_message,
        }),
    );
    messages.push(assistant_message);
}

pub(crate) async fn persist_thread_message(
    store: &dyn AgentStorePort,
    thread_id: &str,
    turn_index: u32,
    message: &ConversationMessage,
) {
    let record = ThreadMessageRecord {
        id: Uuid::new_v4().to_string(),
        thread_id: thread_id.to_owned(),
        turn_index,
        message: message.clone(),
        created_at: Utc::now().to_rfc3339(),
    };
    if let Err(error) = store.insert_thread_message(&record).await {
        warn!(error = %error, thread_id, "failed to persist thread message");
    }
}

async fn persist_turn_state(
    context: &TurnExecutionContext<'_>,
    status: &str,
    messages: Option<&[ConversationMessage]>,
    tool_specs: Option<&[ToolSpec]>,
    response: Option<&crate::port::LlmResponse>,
    error: Option<&str>,
    completed_at: Option<String>,
) {
    let input_messages_json = messages.and_then(|messages| serde_json::to_string(messages).ok());
    let tool_specs_json = tool_specs.and_then(|tool_specs| serde_json::to_string(tool_specs).ok());
    let llm_response_json = response.and_then(|response| serde_json::to_string(response).ok());
    let record = TurnStateRecord {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        status: status.to_owned(),
        input_messages_json,
        tool_specs_json,
        llm_response_json,
        error: error.map(str::to_owned),
        started_at: Utc::now().to_rfc3339(),
        completed_at,
    };
    if let Err(error) = context.store.upsert_turn_state(&record).await {
        warn!(error = %error, thread_id = context.thread_id, "failed to persist turn state");
    }
}

fn insert_injected_messages(
    messages: &mut Vec<ConversationMessage>,
    injected: Vec<ConversationMessage>,
) {
    if injected.is_empty() {
        return;
    }
    let insert_at = messages
        .iter()
        .position(|message| message.role != "system" && message.role != "developer")
        .unwrap_or(messages.len());
    for (offset, message) in injected.into_iter().enumerate() {
        messages.insert(insert_at + offset, message);
    }
}

fn append_observations(messages: &mut Vec<ConversationMessage>, observations: Vec<String>) {
    for observation in observations.into_iter().filter(|value| !value.trim().is_empty()) {
        messages.push(ConversationMessage {
            role: "developer".to_owned(),
            content: ConversationMessageContent::Text(format!(
                "Local hook observation:\n{observation}"
            )),
            name: Some("slab_hook".to_owned()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        });
    }
}

async fn emit_turn_metrics(
    context: &TurnExecutionContext<'_>,
    started_at: std::time::Instant,
    success: bool,
) {
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseMetrics {
                    metrics: AgentMetrics {
                        name: "agent_turn".to_owned(),
                        duration_ms: started_at.elapsed().as_millis(),
                        success: Some(success),
                    },
                },
            },
        )
        .await;
}

fn assistant_item_id(turn_index: u32) -> String {
    format!("assistant-{turn_index}")
}

fn tool_specs_trace_payload(tool_specs: &[ToolSpec]) -> serde_json::Value {
    serde_json::Value::Array(
        tool_specs
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters_schema": tool.parameters_schema,
                })
            })
            .collect(),
    )
}

fn parsed_tool_calls_trace_payload(tool_calls: &[ParsedToolCall]) -> serde_json::Value {
    serde_json::Value::Array(
        tool_calls
            .iter()
            .map(|tool_call| {
                serde_json::json!({
                    "id": tool_call.id,
                    "name": tool_call.name,
                    "arguments": tool_call.arguments,
                })
            })
            .collect(),
    )
}

struct TurnTextDeltaObserver<'a> {
    thread_id: &'a str,
    turn_index: u32,
    notify: &'a dyn AgentNotifyPort,
}

#[async_trait]
impl LlmStreamObserver for TurnTextDeltaObserver<'_> {
    async fn on_text_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseOutputTextDelta {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        delta: delta.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }

    async fn on_reasoning_delta(&mut self, delta: &str) -> Result<(), AgentError> {
        if delta.is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseReasoningTextDelta {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        delta: delta.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }

    async fn on_reasoning_done(&mut self, text: &str) -> Result<(), AgentError> {
        if text.trim().is_empty() {
            return Ok(());
        }

        self.notify
            .on_turn_event(
                self.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(self.turn_index),
                    event: AgentEventKind::ResponseReasoningTextDone {
                        item_id: assistant_item_id(self.turn_index),
                        output_index: 0,
                        content_index: 0,
                        text: text.to_owned(),
                    },
                },
            )
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use slab_types::{ConversationMessage, ConversationMessageContent};

    use crate::event::AgentArtifactKind;

    use super::{collect_turn_artifact_refs, is_external_tool_name};

    #[test]
    fn is_external_tool_name_classifies_offline_droppable_tools() {
        for external in ["web_search", "mcp_call", "mcp_list_tools", "mcp__server__tool"] {
            assert!(is_external_tool_name(external), "{external} should be external");
        }
        for local in [
            "read_file",
            "write_file",
            "shell",
            "grep",
            "plan_update",
            "task.complete",
            "verify",
            "workspace.open",
        ] {
            assert!(!is_external_tool_name(local), "{local} should stay available offline");
        }
    }

    fn message(role: &str, content: &str) -> ConversationMessage {
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(content.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    #[test]
    fn collect_turn_artifact_refs_uses_only_latest_tool_block() {
        let messages = vec![
            message("tool", r#"{"surface":"workspace","revealPath":"old.rs","opened":true}"#),
            message("assistant", "older answer"),
            message("tool", r#"{"surface":"workspace","revealPath":"src\\main.rs","opened":true}"#),
            message("tool", r#"{"surface":"review","path":"src/lib.rs","opened":true}"#),
        ];

        let refs = collect_turn_artifact_refs(&messages);

        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].path, "src/main.rs");
        assert_eq!(refs[0].kind, AgentArtifactKind::File);
        assert_eq!(refs[1].path, "src/lib.rs");
        assert_eq!(refs[1].kind, AgentArtifactKind::Diff);
    }

    #[test]
    fn collect_turn_artifact_refs_drops_unsafe_and_non_path_payloads() {
        let messages = vec![
            message(
                "tool",
                r#"{"surface":"workspace","revealPath":"C:/Users/example/.ssh/id_rsa","opened":true}"#,
            ),
            message(
                "tool",
                r#"{"surface":"review","diff":"diff --git a/src/lib.rs b/src/lib.rs","opened":true}"#,
            ),
            message("tool", r#"{"surface":"image","prompt":"draw a logo","opened":true}"#),
            message(
                "tool",
                r#"{"surface":"workspace","revealPath":"../outside.rs","opened":true}"#,
            ),
        ];

        let refs = collect_turn_artifact_refs(&messages);

        assert!(refs.is_empty());
    }
}
