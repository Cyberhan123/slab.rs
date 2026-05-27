//! Single-turn execution logic (private to the crate).

use std::sync::Arc;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use slab_types::{
    ConversationMessage, ConversationMessageContent, ConversationToolCall,
    ConversationToolFunction, agent::ToolCallStatus,
};

use crate::{
    config::AgentConfig,
    error::AgentError,
    event::{AgentEventKind, AgentMetrics, ToolExecutionStatus},
    hook::{AgentHook, HookEvent, HookOutcome, dispatch_hooks},
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalDecision, ApprovalPort, LlmPort,
        ThreadMessageRecord, ToolCallRecord, TurnEvent,
    },
    risk::ToolRiskAnalyzer,
    tool::{ToolContext, ToolHandler, ToolRouter},
};

/// Execute a single LLM turn.
///
/// Returns `true` if another turn is needed (i.e. the model emitted tool
/// calls), or `false` when the model produced a final answer.
pub(crate) struct TurnExecutionContext<'a> {
    pub thread_id: &'a str,
    pub turn_index: u32,
    pub depth: u32,
    pub config: &'a AgentConfig,
    pub llm: &'a dyn LlmPort,
    pub tools: &'a ToolRouter,
    pub store: &'a dyn AgentStorePort,
    pub notify: &'a dyn AgentNotifyPort,
    pub approval: &'a dyn ApprovalPort,
    pub hooks: &'a [Arc<dyn AgentHook>],
    pub risk: &'a dyn ToolRiskAnalyzer,
    pub cancellation: &'a CancellationToken,
}

pub(crate) async fn execute_turn(
    context: TurnExecutionContext<'_>,
    messages: &mut Vec<ConversationMessage>,
) -> Result<bool, AgentError> {
    let turn_started_at = std::time::Instant::now();
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    // Build the list of allowed tool specs for this turn.
    let tool_specs: Vec<_> = if context.config.allowed_tools.is_empty() {
        context.tools.tool_specs()
    } else {
        context
            .tools
            .tool_specs()
            .into_iter()
            .filter(|s| context.config.allowed_tools.contains(&s.name))
            .collect()
    };

    debug!(thread_id = context.thread_id, turn_index = context.turn_index, "executing turn");

    let response = tokio::select! {
        response = context.llm.chat_completion(
            &context.config.model,
            messages,
            &tool_specs,
            context.config,
        ) => response?,
        _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
    };
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    if response.tool_calls.is_empty() {
        // Model produced a final answer — no more turns required.
        let content = response.content.unwrap_or_default();
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
        persist_thread_message(context.store, context.thread_id, context.turn_index, &message)
            .await;
        messages.push(message);
        emit_turn_metrics(&context, turn_started_at, true).await;
        return Ok(false);
    }

    // Emit assistant delta for any text alongside tool calls.
    if let Some(ref text) = response.content
        && !text.is_empty()
    {
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
                        delta: text.clone(),
                    },
                },
            )
            .await;
    }

    // Model requested tool calls — build the assistant message and execute.
    let assistant_tool_calls: Vec<ConversationToolCall> = response
        .tool_calls
        .iter()
        .map(|tc| ConversationToolCall {
            id: Some(tc.id.clone()),
            r#type: "function".to_owned(),
            function: ConversationToolFunction {
                name: tc.name.clone(),
                arguments: tc.arguments.clone(),
            },
        })
        .collect();

    let assistant_message = ConversationMessage {
        role: "assistant".to_owned(),
        content: ConversationMessageContent::Text(response.content.unwrap_or_default()),
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
    messages.push(assistant_message);

    let ctx = ToolContext {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        depth: context.depth,
    };
    let now = Utc::now().to_rfc3339();

    for tc in &response.tool_calls {
        if context.cancellation.is_cancelled() {
            return Err(AgentError::Interrupted);
        }
        let call_id = Uuid::new_v4().to_string();

        // Parse arguments first so hooks receive a structured Value.
        let parsed_args = match serde_json::from_str::<serde_json::Value>(&tc.arguments) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    thread_id = context.thread_id,
                    tool = %tc.name,
                    error = %e,
                    "failed to parse tool call arguments as JSON"
                );
                let err_msg = format!("invalid tool call arguments: {e}");
                context
                    .notify
                    .on_turn_event(
                        context.thread_id,
                        &TurnEvent::Response {
                            turn_index: Some(context.turn_index),
                            event: AgentEventKind::ResponseToolCallOutput {
                                item_id: tc.id.clone(),
                                call_id: call_id.clone(),
                                output: err_msg.clone(),
                                status: ToolExecutionStatus::Failed,
                            },
                        },
                    )
                    .await;
                let message = ConversationMessage {
                    role: "tool".to_owned(),
                    content: ConversationMessageContent::Text(err_msg),
                    name: None,
                    tool_call_id: Some(tc.id.clone()),
                    tool_calls: vec![],
                };
                persist_thread_message(
                    context.store,
                    context.thread_id,
                    context.turn_index,
                    &message,
                )
                .await;
                messages.push(message);
                continue;
            }
        };

        // Run PreToolUse hooks — may block or modify args.
        let effective_args = {
            let pre_event = HookEvent::PreToolUse {
                tool_name: tc.name.clone(),
                arguments: parsed_args.clone(),
            };
            match dispatch_hooks(context.hooks, &pre_event).await {
                HookOutcome::Block { reason } => {
                    warn!(
                        thread_id = context.thread_id,
                        tool = %tc.name,
                        reason,
                        "tool call blocked by hook"
                    );
                    context
                        .notify
                        .on_turn_event(
                            context.thread_id,
                            &TurnEvent::Response {
                                turn_index: Some(context.turn_index),
                                event: AgentEventKind::ResponseToolCallOutput {
                                    item_id: tc.id.clone(),
                                    call_id: call_id.clone(),
                                    output: reason.clone(),
                                    status: ToolExecutionStatus::Failed,
                                },
                            },
                        )
                        .await;
                    let message = ConversationMessage {
                        role: "tool".to_owned(),
                        content: ConversationMessageContent::Text(reason),
                        name: None,
                        tool_call_id: Some(tc.id.clone()),
                        tool_calls: vec![],
                    };
                    persist_thread_message(
                        context.store,
                        context.thread_id,
                        context.turn_index,
                        &message,
                    )
                    .await;
                    messages.push(message);
                    continue;
                }
                HookOutcome::ModifyArgs { arguments } => arguments,
                HookOutcome::Continue => parsed_args,
            }
        };
        let risk = context.risk.analyze(&tc.name, &effective_args).await;

        // Emit ToolCallStarted AFTER hooks so SSE consumers see the final
        // effective arguments (hooks may have modified them).
        context
            .notify
            .on_turn_event(
                context.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(context.turn_index),
                    event: AgentEventKind::ResponseFunctionCallArgumentsDone {
                        item_id: tc.id.clone(),
                        call_id: call_id.clone(),
                        name: tc.name.clone(),
                        output_index: 0,
                        arguments: serde_json::to_string(&effective_args)
                            .unwrap_or_else(|_| tc.arguments.clone()),
                        risk: Some(risk.clone()),
                    },
                },
            )
            .await;

        let handler = context.tools.get(&tc.name);
        let approval_request =
            handler.and_then(|handler| handler.approval_request(&effective_args));
        let initial_status = if approval_request.is_some() {
            ToolCallStatus::Pending
        } else {
            ToolCallStatus::Running
        };

        let record = ToolCallRecord {
            id: call_id.clone(),
            thread_id: context.thread_id.to_owned(),
            tool_name: tc.name.clone(),
            arguments: tc.arguments.clone(),
            output: None,
            status: initial_status,
            created_at: now.clone(),
            completed_at: None,
        };

        if let Err(e) = context.store.insert_tool_call(&record).await {
            warn!(error = %e, call_id, "failed to persist tool call record");
        }

        let (output, call_status) = if let Some(request) = approval_request {
            let decision = tokio::select! {
                decision = context.approval.request_approval(
                    context.thread_id,
                    &call_id,
                    &tc.name,
                    &request.command,
                    Some(risk.clone()),
                ) => decision,
                _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
            };
            match decision {
                ApprovalDecision::Approved => {
                    context
                        .notify
                        .on_turn_event(
                            context.thread_id,
                            &TurnEvent::Response {
                                turn_index: Some(context.turn_index),
                                event: AgentEventKind::ResponseToolCallApprovalResolved {
                                    item_id: tc.id.clone(),
                                    call_id: call_id.clone(),
                                    tool_name: tc.name.clone(),
                                    approved: true,
                                },
                            },
                        )
                        .await;
                    if context.cancellation.is_cancelled() {
                        return Err(AgentError::Interrupted);
                    }
                    tokio::select! {
                        result = execute_tool_call(&tc.name, handler, &ctx, &effective_args) => result,
                        _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
                    }
                }
                ApprovalDecision::Rejected => {
                    context
                        .notify
                        .on_turn_event(
                            context.thread_id,
                            &TurnEvent::Response {
                                turn_index: Some(context.turn_index),
                                event: AgentEventKind::ResponseToolCallApprovalResolved {
                                    item_id: tc.id.clone(),
                                    call_id: call_id.clone(),
                                    tool_name: tc.name.clone(),
                                    approved: false,
                                },
                            },
                        )
                        .await;
                    ("tool call rejected by approval policy".to_string(), ToolCallStatus::Failed)
                }
            }
        } else {
            if context.cancellation.is_cancelled() {
                return Err(AgentError::Interrupted);
            }
            tokio::select! {
                result = execute_tool_call(&tc.name, handler, &ctx, &effective_args) => result,
                _ = context.cancellation.cancelled() => return Err(AgentError::Interrupted),
            }
        };
        if context.cancellation.is_cancelled() {
            return Err(AgentError::Interrupted);
        }

        // Run PostToolUse hooks.
        let post_event = HookEvent::PostToolUse {
            tool_name: tc.name.clone(),
            arguments: effective_args,
            output: output.clone(),
        };
        dispatch_hooks(context.hooks, &post_event).await;

        // Emit ToolCallOutput event.
        context
            .notify
            .on_turn_event(
                context.thread_id,
                &TurnEvent::Response {
                    turn_index: Some(context.turn_index),
                    event: AgentEventKind::ResponseToolCallOutput {
                        item_id: tc.id.clone(),
                        call_id: call_id.clone(),
                        output: output.clone(),
                        status: match call_status {
                            ToolCallStatus::Pending | ToolCallStatus::Running => {
                                ToolExecutionStatus::Failed
                            }
                            ToolCallStatus::Completed => ToolExecutionStatus::Completed,
                            ToolCallStatus::Failed => ToolExecutionStatus::Failed,
                        },
                    },
                },
            )
            .await;

        let completed_at = Utc::now().to_rfc3339();
        if let Err(e) = context
            .store
            .update_tool_call(&call_id, Some(&output), call_status, &completed_at)
            .await
        {
            warn!(error = %e, call_id, "failed to update tool call record");
        }

        let message = ConversationMessage {
            role: "tool".to_owned(),
            content: ConversationMessageContent::Text(output),
            name: None,
            tool_call_id: Some(tc.id.clone()),
            tool_calls: vec![],
        };
        persist_thread_message(context.store, context.thread_id, context.turn_index, &message)
            .await;
        messages.push(message);
    }

    emit_turn_metrics(&context, turn_started_at, true).await;
    Ok(true)
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
    if let Err(e) = store.insert_thread_message(&record).await {
        warn!(error = %e, thread_id, "failed to persist thread message");
    }
}

async fn execute_tool_call(
    tool_name: &str,
    handler: Option<&dyn ToolHandler>,
    ctx: &ToolContext,
    arguments: &serde_json::Value,
) -> (String, ToolCallStatus) {
    let Some(handler) = handler else {
        warn!(tool = tool_name, "tool not found");
        return (format!("tool not found: {tool_name}"), ToolCallStatus::Failed);
    };

    match handler.execute(ctx, arguments).await {
        Ok(out) => (out.content, ToolCallStatus::Completed),
        Err(e) => {
            warn!(tool = handler.name(), error = %e, "tool execution failed");
            (e.to_string(), ToolCallStatus::Failed)
        }
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
