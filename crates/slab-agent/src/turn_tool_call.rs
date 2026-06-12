//! Tool-call execution for a single agent turn.

use std::sync::Arc;

use chrono::Utc;
use futures::future::join_all;
use tracing::{info, warn};
use uuid::Uuid;

use slab_agent_tracing::record_json;
use slab_types::{ConversationMessage, agent::ToolCallStatus};

use crate::{
    error::AgentError,
    event::{AgentEventKind, ToolRiskAssessment},
    hook::{HookEvent, HookToolAction, dispatch_registered_hooks},
    port::{ApprovalDecision, ParsedToolCall, TurnEvent},
    state::ToolCallStateMachine,
    tool::{ToolApprovalRequest, ToolContext, ToolHandler},
    turn::TurnExecutionContext,
    turn_tool_record::{
        insert_tool_call_record, persist_tool_message_record,
        record_failed_tool_call_without_persisting_message, tool_execution_status,
        update_tool_call_record, update_tool_call_status,
    },
};

struct ToolCallRunResult {
    message: ConversationMessage,
    status: ToolCallStatus,
}

pub(crate) async fn handle_tool_calls(
    context: &TurnExecutionContext<'_>,
    tool_calls: &[ParsedToolCall],
    messages: &mut Vec<ConversationMessage>,
) -> Result<(), AgentError> {
    let tool_context = ToolContext {
        thread_id: context.thread_id.to_owned(),
        turn_index: context.turn_index,
        depth: context.depth,
    };
    let now = Utc::now().to_rfc3339();
    let total = tool_calls.len();
    if total == 0 {
        return Ok(());
    }

    let concurrency = context.config.effective_tool_concurrency().min(total);
    emit_tool_concurrency_started(context, total, concurrency).await;

    let mut results = Vec::with_capacity(total);
    let conversation_context = messages.clone();
    for (chunk_index, chunk) in tool_calls.chunks(concurrency).enumerate() {
        let base_index = chunk_index * concurrency;
        let batch = chunk.iter().enumerate().map(|(offset, tool_call)| {
            let created_at = now.clone();
            let tool_context = tool_context.clone();
            let conversation_messages = conversation_context.as_slice();
            async move {
                handle_tool_call(
                    context,
                    &tool_context,
                    conversation_messages,
                    base_index + offset,
                    tool_call,
                    &created_at,
                )
                .await
            }
        });
        results.extend(join_all(batch).await);
    }

    let mut completed = 0usize;
    let mut failed = 0usize;
    let mut first_error = None;
    for result in results {
        match result {
            Ok(result) => {
                match result.status {
                    ToolCallStatus::Completed => completed += 1,
                    ToolCallStatus::Pending | ToolCallStatus::Running | ToolCallStatus::Failed => {
                        failed += 1;
                    }
                }
                persist_tool_message_record(context, result.message, messages).await;
            }
            Err(error) => {
                failed += 1;
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    emit_tool_concurrency_completed(context, total, completed, failed).await;
    if let Some(error) = first_error {
        return Err(error);
    }
    Ok(())
}

async fn emit_tool_concurrency_started(
    context: &TurnExecutionContext<'_>,
    total: usize,
    concurrency: usize,
) {
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_concurrency_started",
        serde_json::json!({
            "total": total,
            "concurrency": concurrency,
        }),
    );
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseToolCallConcurrencyStarted { total, concurrency },
            },
        )
        .await;
}

async fn emit_tool_concurrency_completed(
    context: &TurnExecutionContext<'_>,
    total: usize,
    completed: usize,
    failed: usize,
) {
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_concurrency_completed",
        serde_json::json!({
            "total": total,
            "completed": completed,
            "failed": failed,
        }),
    );
    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseToolCallConcurrencyCompleted {
                    total,
                    completed,
                    failed,
                },
            },
        )
        .await;
}

async fn handle_tool_call(
    context: &TurnExecutionContext<'_>,
    tool_context: &ToolContext,
    messages: &[ConversationMessage],
    _index: usize,
    tool_call: &ParsedToolCall,
    created_at: &str,
) -> Result<ToolCallRunResult, AgentError> {
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    let call_id = Uuid::new_v4().to_string();
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_detected",
        serde_json::json!({
            "item_id": tool_call.id,
            "call_id": call_id,
            "tool_name": tool_call.name,
            "arguments": tool_call.arguments,
        }),
    );

    let parsed_args = match serde_json::from_str::<serde_json::Value>(&tool_call.arguments) {
        Ok(value) => value,
        Err(error) => {
            record_json(
                context.trace,
                &context.trace_context,
                "slab-agent",
                "tool_call_arguments_parse_failed",
                serde_json::json!({
                    "item_id": tool_call.id,
                    "call_id": call_id,
                    "tool_name": tool_call.name,
                    "arguments": tool_call.arguments,
                    "error": error.to_string(),
                }),
            );
            info!(
                thread_id = context.thread_id,
                turn_index = context.turn_index,
                item_id = %tool_call.id,
                tool_name = %tool_call.name,
                arguments = %tool_call.arguments,
                error = %error,
                "agent tool call arguments parse failed"
            );
            warn!(
                thread_id = context.thread_id,
                tool = %tool_call.name,
                error = %error,
                "failed to parse tool call arguments as JSON"
            );
            let output = format!("invalid tool call arguments: {error}");
            let message = record_failed_tool_call_without_persisting_message(
                context, &call_id, tool_call, output, created_at,
            )
            .await?;
            return Ok(ToolCallRunResult { message, status: ToolCallStatus::Failed });
        }
    };
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_arguments_parsed",
        serde_json::json!({
            "item_id": tool_call.id,
            "call_id": call_id,
            "tool_name": tool_call.name,
            "arguments": parsed_args,
        }),
    );

    let pre_event = HookEvent::OnToolStart {
        thread_id: context.thread_id.to_owned(),
        session_id: context.session_id.to_owned(),
        turn_index: context.turn_index,
        messages: messages.to_vec(),
        call_id: call_id.clone(),
        tool_name: tool_call.name.clone(),
        arguments: parsed_args.clone(),
    };
    let pre_effects = dispatch_registered_hooks(context.hooks, &pre_event).await;
    let pre_observations = pre_effects.observations;
    let effective_args = match pre_effects.tool_action {
        HookToolAction::Block { reason } => {
            let mut output = reason.clone();
            append_hook_observations(&mut output, pre_observations);
            record_json(
                context.trace,
                &context.trace_context,
                "slab-agent",
                "tool_call_blocked",
                serde_json::json!({
                    "item_id": tool_call.id,
                    "call_id": call_id,
                    "tool_name": tool_call.name,
                    "reason": reason,
                }),
            );
            warn!(
                thread_id = context.thread_id,
                tool = %tool_call.name,
                reason = %output,
                "tool call blocked by hook"
            );
            let message = record_failed_tool_call_without_persisting_message(
                context, &call_id, tool_call, output, created_at,
            )
            .await?;
            return Ok(ToolCallRunResult { message, status: ToolCallStatus::Failed });
        }
        HookToolAction::ModifyArgs { arguments } => arguments,
        HookToolAction::Continue => parsed_args,
    };

    let risk = context.risk.analyze(&tool_call.name, &effective_args).await;
    let effective_arguments =
        serde_json::to_string(&effective_args).unwrap_or_else(|_| tool_call.arguments.clone());
    info!(
        thread_id = context.thread_id,
        turn_index = context.turn_index,
        item_id = %tool_call.id,
        call_id = %call_id,
        tool_name = %tool_call.name,
        arguments = %effective_arguments,
        "agent function call arguments done"
    );

    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseFunctionCallArgumentsDone {
                    item_id: tool_call.id.clone(),
                    call_id: call_id.clone(),
                    name: tool_call.name.clone(),
                    output_index: 0,
                    arguments: effective_arguments.clone(),
                    risk: Some(risk.clone()),
                },
            },
        )
        .await;

    let handler = context.tools.get(&tool_call.name);
    let approval_request =
        handler.as_ref().and_then(|handler| handler.approval_request(&effective_args));
    let initial_status =
        if approval_request.is_some() { ToolCallStatus::Pending } else { ToolCallStatus::Running };
    let mut tool_state = ToolCallStateMachine::new(initial_status);
    insert_tool_call_record(context, &call_id, tool_call, tool_state.status(), created_at).await;

    let (mut output, call_status) = run_tool_with_optional_approval(ToolRunContext {
        context,
        call_id: &call_id,
        tool_call,
        tool_context,
        effective_args: &effective_args,
        effective_arguments: &effective_arguments,
        risk: &risk,
        handler,
        approval_request,
        tool_state: &mut tool_state,
    })
    .await?;
    let call_status = tool_state.transition(call_status)?;
    if context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }

    info!(
        thread_id = context.thread_id,
        turn_index = context.turn_index,
        item_id = %tool_call.id,
        call_id = %call_id,
        tool_name = %tool_call.name,
        status = ?call_status,
        output_len = output.len(),
        "agent tool call output"
    );
    record_json(
        context.trace,
        &context.trace_context,
        "slab-agent",
        "tool_call_output",
        serde_json::json!({
            "item_id": tool_call.id,
            "call_id": call_id,
            "tool_name": tool_call.name,
            "status": call_status,
            "output": output,
        }),
    );
    append_hook_observations(&mut output, pre_observations);

    let post_event = HookEvent::OnToolEnd {
        thread_id: context.thread_id.to_owned(),
        session_id: context.session_id.to_owned(),
        turn_index: context.turn_index,
        messages: messages.to_vec(),
        call_id: call_id.clone(),
        tool_name: tool_call.name.clone(),
        arguments: effective_args,
        output: output.clone(),
        status: call_status,
    };
    let post_effects = dispatch_registered_hooks(context.hooks, &post_event).await;
    append_hook_observations(&mut output, post_effects.observations);

    context
        .notify
        .on_turn_event(
            context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(context.turn_index),
                event: AgentEventKind::ResponseToolCallOutput {
                    item_id: tool_call.id.clone(),
                    call_id: call_id.clone(),
                    output: output.clone(),
                    status: tool_execution_status(call_status),
                },
            },
        )
        .await;

    update_tool_call_record(context, &call_id, Some(&output), call_status).await;
    let message = crate::turn_tool_record::tool_message(tool_call, output);

    Ok(ToolCallRunResult { message, status: call_status })
}

struct ToolRunContext<'a, 'ctx> {
    context: &'a TurnExecutionContext<'ctx>,
    call_id: &'a str,
    tool_call: &'a ParsedToolCall,
    tool_context: &'a ToolContext,
    effective_args: &'a serde_json::Value,
    effective_arguments: &'a str,
    risk: &'a ToolRiskAssessment,
    handler: Option<Arc<dyn ToolHandler>>,
    approval_request: Option<ToolApprovalRequest>,
    tool_state: &'a mut ToolCallStateMachine,
}

async fn run_tool_with_optional_approval(
    run: ToolRunContext<'_, '_>,
) -> Result<(String, ToolCallStatus), AgentError> {
    let Some(ref request) = run.approval_request else {
        return run_tool_without_approval(&run).await;
    };

    record_json(
        run.context.trace,
        &run.context.trace_context,
        "slab-agent",
        "tool_call_approval_required",
        serde_json::json!({
            "item_id": run.tool_call.id,
            "call_id": run.call_id,
            "tool_name": run.tool_call.name,
            "command": &request.command,
            "risk": run.risk,
        }),
    );
    info!(
        thread_id = run.context.thread_id,
        turn_index = run.context.turn_index,
        item_id = %run.tool_call.id,
        call_id = %run.call_id,
        tool_name = %run.tool_call.name,
        arguments = %run.effective_arguments,
        "agent tool call approval required"
    );
    let decision = tokio::select! {
        decision = run.context.approval.request_approval(
            run.context.thread_id,
            run.call_id,
            &run.tool_call.name,
            &request.command,
            Some(run.risk.clone()),
        ) => decision,
        _ = run.context.cancellation.cancelled() => return Err(AgentError::Interrupted),
    };

    match decision {
        ApprovalDecision::Approved => {
            emit_approval_resolved(&run, true).await;
            if run.context.cancellation.is_cancelled() {
                return Err(AgentError::Interrupted);
            }
            let running_status = run.tool_state.transition(ToolCallStatus::Running)?;
            update_tool_call_status(run.context, run.call_id, running_status).await;
            emit_tool_execution_started(&run).await;
            Ok(tokio::select! {
                result = execute_tool_call(
                    &run.tool_call.name,
                    run.handler.clone(),
                    run.tool_context,
                    run.effective_args,
                ) => result,
                _ = run.context.cancellation.cancelled() => return Err(AgentError::Interrupted),
            })
        }
        ApprovalDecision::Rejected => {
            emit_approval_resolved(&run, false).await;
            Ok(("tool call rejected by approval policy".to_string(), ToolCallStatus::Failed))
        }
    }
}

async fn run_tool_without_approval(
    run: &ToolRunContext<'_, '_>,
) -> Result<(String, ToolCallStatus), AgentError> {
    if run.context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }
    emit_tool_execution_started(run).await;
    Ok(tokio::select! {
        result = execute_tool_call(
            &run.tool_call.name,
            run.handler.clone(),
            run.tool_context,
            run.effective_args,
        ) => result,
        _ = run.context.cancellation.cancelled() => return Err(AgentError::Interrupted),
    })
}

async fn emit_approval_resolved(run: &ToolRunContext<'_, '_>, approved: bool) {
    record_json(
        run.context.trace,
        &run.context.trace_context,
        "slab-agent",
        "tool_call_approval_resolved",
        serde_json::json!({
            "item_id": run.tool_call.id,
            "call_id": run.call_id,
            "tool_name": run.tool_call.name,
            "approved": approved,
        }),
    );
    info!(
        thread_id = run.context.thread_id,
        turn_index = run.context.turn_index,
        item_id = %run.tool_call.id,
        call_id = %run.call_id,
        tool_name = %run.tool_call.name,
        status = if approved { "approved" } else { "rejected" },
        "agent tool call approval resolved"
    );
    run.context
        .notify
        .on_turn_event(
            run.context.thread_id,
            &TurnEvent::Response {
                turn_index: Some(run.context.turn_index),
                event: AgentEventKind::ResponseToolCallApprovalResolved {
                    item_id: run.tool_call.id.clone(),
                    call_id: run.call_id.to_owned(),
                    tool_name: run.tool_call.name.clone(),
                    approved,
                },
            },
        )
        .await;
}

async fn emit_tool_execution_started(run: &ToolRunContext<'_, '_>) {
    info!(
        thread_id = run.context.thread_id,
        turn_index = run.context.turn_index,
        item_id = %run.tool_call.id,
        call_id = %run.call_id,
        tool_name = %run.tool_call.name,
        arguments = %run.effective_arguments,
        "agent tool call execution started"
    );
    record_json(
        run.context.trace,
        &run.context.trace_context,
        "slab-agent",
        "tool_call_started",
        serde_json::json!({
            "item_id": run.tool_call.id,
            "call_id": run.call_id,
            "tool_name": run.tool_call.name,
            "arguments": run.effective_args,
        }),
    );
}

async fn execute_tool_call(
    tool_name: &str,
    handler: Option<Arc<dyn ToolHandler>>,
    ctx: &ToolContext,
    arguments: &serde_json::Value,
) -> (String, ToolCallStatus) {
    let Some(handler) = handler else {
        info!(tool_name = %tool_name, "agent tool call handler not found");
        warn!(tool = tool_name, "tool not found");
        return (format!("tool not found: {tool_name}"), ToolCallStatus::Failed);
    };

    match handler.execute(ctx, arguments).await {
        Ok(output) => (output.content, ToolCallStatus::Completed),
        Err(error) => {
            warn!(tool = handler.name(), error = %error, "tool execution failed");
            (error.to_string(), ToolCallStatus::Failed)
        }
    }
}

fn append_hook_observations(output: &mut String, observations: Vec<String>) {
    let observations = observations
        .into_iter()
        .filter(|observation| !observation.trim().is_empty())
        .collect::<Vec<_>>();
    if observations.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("\nHook observations:\n");
    for observation in observations {
        output.push_str("- ");
        output.push_str(observation.trim());
        output.push('\n');
    }
}
