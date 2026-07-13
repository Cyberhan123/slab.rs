//! Tool-call execution for a single agent turn.

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use futures::future::join_all;
use tracing::{info, warn};
use uuid::Uuid;

use slab_agent_tracing::record_json;
use slab_types::{ConversationMessage, agent::ToolCallStatus};

use crate::{
    error::AgentError,
    hook::{HookEvent, HookToolAction, dispatch_registered_hooks},
    port::{ApprovalDecision, ParsedToolCall, ToolRiskAssessment},
    protocol::{
        CommandExecutionRequestApprovalParams, EventMsg, ItemCompletedParams, ItemStartedParams,
        TurnItem,
    },
    state::ToolCallStateMachine,
    tool::{PlanRef, ToolApprovalRequest, ToolContext, ToolHandler, ToolOutput},
    turn::TurnExecutionContext,
    turn_tool_record::{
        insert_tool_call_record, persist_tool_message_record,
        record_failed_tool_call_without_persisting_message, update_tool_call_record,
        update_tool_call_status,
    },
};

/// Tool name that signals structured task completion. Mirrors
/// `slab_agent_tools::TASK_COMPLETE_TOOL_NAME`; duplicated here because
/// `slab-agent` cannot depend on `slab-agent-tools` (dependency direction is
/// reversed). The producer owns the metadata shape; see
/// `crates/slab-agent-tools/src/task_complete.rs`.
const TASK_COMPLETE_TOOL_NAME: &str = "task.complete";
/// Metadata key the `task.complete` tool places its completion marker under.
/// Mirrors `slab_agent_tools::TASK_COMPLETE_METADATA_KEY`.
const TASK_COMPLETE_METADATA_KEY: &str = "task_complete";

/// Structured completion payload extracted from a successful `task.complete`
/// tool call. Consumed by the turn loop to emit the final answer (双轨 2).
#[derive(Debug, Clone)]
pub(crate) struct TaskCompletion {
    pub summary: String,
}

/// Parse a [`TaskCompletion`] out of a tool's metadata marker, when the tool
/// that just ran is `task.complete` and it succeeded.
fn parse_task_completion(metadata: Option<&serde_json::Value>) -> Option<TaskCompletion> {
    let marker = metadata?.get(TASK_COMPLETE_METADATA_KEY)?;
    let summary = marker.get("summary")?.as_str()?.trim().to_owned();
    if summary.is_empty() {
        return None;
    }
    Some(TaskCompletion { summary })
}

struct ToolCallRunResult {
    message: ConversationMessage,
    status: ToolCallStatus,
    task_completion: Option<TaskCompletion>,
}

// ── Harness-protocol (EventMsg) tool-item emits ───────────────────────────────
//
// slab-agent emits the harness protocol directly (the `EventMsg`/`TurnItem`
// surface in `crate::protocol`). This is what makes tool calls visible to the
// harness WS fan-out and turn-item persistence (bug 1). The legacy
// `AgentEventKind`/`/responses` emits left this crate in slice C3.

/// Workspace root for `CommandExecution.cwd`, or `None` when no workspace is bound.
fn workspace_root_of(context: &TurnExecutionContext<'_>) -> Option<String> {
    context.thread_context.workspace.as_ref().map(|w| w.root.to_string_lossy().into_owned())
}

/// Split an `mcp__{server}__{tool}` proxy name into `(server, tool)`.
///
/// `proxy_tool_name` formats with exactly two `__` separators, so `splitn(3,
/// "__")` is the reversible parse. Falls back to `("<unknown>", name)` when the
/// name is malformed. Server/tool are display-only on the wire.
fn parse_mcp_proxy_name(name: &str) -> (String, String) {
    let mut parts = name.splitn(3, "__");
    let _ = parts.next(); // leading "mcp"
    match (parts.next(), parts.next()) {
        (Some(server), Some(tool)) if !server.is_empty() => (server.to_owned(), tool.to_owned()),
        _ => ("<unknown>".to_owned(), name.to_owned()),
    }
}

/// Build the harness `TurnItem` for a tool call.
///
/// `status` is `"running"` for `ItemStarted`, `"completed"`/`"failed"` for
/// `ItemCompleted`. `output` is the tool result text (filled only on
/// completion). The item id is the provider-assigned `tool_call.id`. Unknown /
/// read-only tools fall back to `CommandExecution` so every tool call is
/// visible on the harness timeline (no new wire variant).
fn tool_turn_item(
    tool_call: &ParsedToolCall,
    args: &serde_json::Value,
    status: &str,
    output: Option<&str>,
    workspace_root: Option<&str>,
) -> TurnItem {
    let id = tool_call.id.clone();
    let status = status.to_owned();
    match tool_call.name.as_str() {
        "shell" => TurnItem::CommandExecution {
            id,
            command: args
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned(),
            cwd: workspace_root.unwrap_or("").to_owned(),
            process_id: None,
            status,
            aggregated_output: output.map(str::to_owned),
            exit_code: None,
            duration_ms: None,
        },
        "write_file" => TurnItem::FileChange {
            id,
            changes: vec![serde_json::json!({
                "path": args.get("path").and_then(serde_json::Value::as_str).unwrap_or(""),
                "type": "edit",
            })],
            status,
        },
        "apply_patch" => {
            let patch = args.get("patch").and_then(serde_json::Value::as_str).unwrap_or("");
            TurnItem::FileChange {
                id,
                changes: vec![serde_json::json!({
                    "path": first_path_in_patch(patch),
                    "type": "edit",
                    "diff": patch,
                })],
                status,
            }
        }
        "web_search" => TurnItem::WebSearch {
            id,
            query: args.get("query").and_then(serde_json::Value::as_str).unwrap_or("").to_owned(),
        },
        "mcp_call" => TurnItem::McpToolCall {
            id,
            server: args
                .get("server")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>")
                .to_owned(),
            tool: args
                .get("tool")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<unknown>")
                .to_owned(),
            arguments: args.get("arguments").cloned().unwrap_or_else(|| args.clone()),
            status,
            result: output.and_then(|o| serde_json::from_str(o).ok()),
            error: None,
            duration_ms: None,
        },
        name if name.starts_with("mcp__") => {
            let (server, tool) = parse_mcp_proxy_name(name);
            TurnItem::McpToolCall {
                id,
                server,
                tool,
                arguments: args.clone(),
                status,
                result: output.and_then(|o| serde_json::from_str(o).ok()),
                error: None,
                duration_ms: None,
            }
        }
        // Fallback: every other tool (read_file/grep/plan/verify/task.complete/…)
        // maps to CommandExecution so it is visible on the harness timeline.
        _ => TurnItem::CommandExecution {
            id,
            command: tool_call.name.clone(),
            cwd: String::new(),
            process_id: None,
            status,
            aggregated_output: output.map(str::to_owned),
            exit_code: None,
            duration_ms: None,
        },
    }
}

/// Emit `EventMsg::ItemStarted` for `item` on the harness channel.
async fn emit_item_started(context: &TurnExecutionContext<'_>, item: TurnItem) {
    let msg = EventMsg::ItemStarted(ItemStartedParams {
        item,
        thread_id: context.thread_id.to_owned(),
        turn_id: context.turn_index.to_string(),
    });
    context.notify.on_event_msg(context.thread_id, &msg).await;
}

/// Emit `EventMsg::ItemCompleted` for `item` on the harness channel.
async fn emit_item_completed(context: &TurnExecutionContext<'_>, item: TurnItem) {
    let msg = EventMsg::ItemCompleted(ItemCompletedParams {
        item,
        thread_id: context.thread_id.to_owned(),
        turn_id: context.turn_index.to_string(),
    });
    context.notify.on_event_msg(context.thread_id, &msg).await;
}

/// Emit a well-formed `ItemStarted` + `ItemCompleted(failed)` pair for a tool
/// call that never reaches the normal execution path (argument-parse failure,
/// hook block, policy deny, invalid tool call).
///
/// Every `item_id` the client ever observes must have a start→complete
/// lifecycle; callers that bail before the success-path `ItemStarted` use this
/// so a lone `ItemCompleted` never appears for an unseen item.
pub(crate) async fn emit_tool_item_failed(
    context: &TurnExecutionContext<'_>,
    tool_call: &ParsedToolCall,
    args: &serde_json::Value,
    output: &str,
) {
    let workspace_root = workspace_root_of(context);
    let started = tool_turn_item(tool_call, args, "running", None, workspace_root.as_deref());
    emit_item_started(context, started).await;
    let completed =
        tool_turn_item(tool_call, args, "failed", Some(output), workspace_root.as_deref());
    emit_item_completed(context, completed).await;
}

/// The full set of persistence scopes a client may offer when approving.
/// Mirrors the harness projection's `default_allowed_scopes` so the approval
/// banner offers the same choices on the `EventMsg` path.
fn default_allowed_scopes() -> Vec<slab_exec_policy::ApprovalScope> {
    use slab_exec_policy::ApprovalScope;
    vec![
        ApprovalScope::RunOnce,
        ApprovalScope::AlwaysInWorkspace,
        ApprovalScope::Always,
        ApprovalScope::Deny,
    ]
}

/// Emit `EventMsg::CommandExecutionRequestApproval` for a tool that the
/// exec-policy engine gated behind approval. `item_id` is the per-call UUID
/// (`call_id`) — the same key the approval resolution flow correlates on, so
/// the client can match the banner back to the pending decision.
async fn emit_approval_request(run: &ToolRunContext<'_, '_>) {
    let Some(request) = &run.approval_request else {
        return;
    };
    let msg = EventMsg::CommandExecutionRequestApproval(CommandExecutionRequestApprovalParams {
        thread_id: run.context.thread_id.to_owned(),
        turn_id: run.context.turn_index.to_string(),
        item_id: run.call_id.to_owned(),
        command: request.display.clone(),
        cwd: String::new(),
        reason: None,
        category: Some(request.descriptor.category),
        allowed_scopes: default_allowed_scopes(),
    });
    run.context.notify.on_event_msg(run.context.thread_id, &msg).await;
}

/// Execute the given tool calls and persist their results.
///
/// Returns `Some(TaskCompletion)` when a `task.complete` tool call succeeded,
/// signalling the turn loop to emit the final answer (双轨 2). Returns `None`
/// for a normal tool-call turn that should continue to the next LLM turn.
pub(crate) async fn handle_tool_calls(
    context: &TurnExecutionContext<'_>,
    tool_calls: &[ParsedToolCall],
    messages: &mut Vec<ConversationMessage>,
) -> Result<Option<TaskCompletion>, AgentError> {
    let mut tool_context_builder = ToolContext::for_thread(context.thread_id)
        .turn_index(context.turn_index)
        .depth(context.depth);
    if let Some(workspace) = context.thread_context.workspace.as_ref() {
        let mut workspace = workspace.clone();
        if workspace.session_id.is_none() {
            workspace.session_id = Some(context.session_id.to_owned());
        }
        tool_context_builder = tool_context_builder.workspace(workspace);
    }
    if let Some(plan_id) = context.thread_context.plan_id.as_deref().map(str::trim)
        && !plan_id.is_empty()
    {
        tool_context_builder = tool_context_builder.plan(PlanRef {
            thread_id: context.thread_id.to_owned(),
            plan_id: Some(plan_id.to_owned()),
        });
    }
    let tool_context = tool_context_builder.build();
    let now = Utc::now().to_rfc3339();
    let total = tool_calls.len();
    if total == 0 {
        return Ok(None);
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
    let mut task_completion = None;
    for result in results {
        match result {
            Ok(result) => {
                match result.status {
                    ToolCallStatus::Completed => completed += 1,
                    ToolCallStatus::Pending | ToolCallStatus::Running | ToolCallStatus::Failed => {
                        failed += 1;
                    }
                }
                // A successful `task.complete` wins; last one wins if multiple.
                if result.task_completion.is_some() {
                    task_completion = result.task_completion;
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
    Ok(task_completion)
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
            emit_tool_item_failed(context, tool_call, &serde_json::Value::Null, &output).await;
            let message = record_failed_tool_call_without_persisting_message(
                context, &call_id, tool_call, output, created_at,
            )
            .await?;
            return Ok(ToolCallRunResult {
                message,
                status: ToolCallStatus::Failed,
                task_completion: None,
            });
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
            emit_tool_item_failed(context, tool_call, &parsed_args, &output).await;
            let message = record_failed_tool_call_without_persisting_message(
                context, &call_id, tool_call, output, created_at,
            )
            .await?;
            return Ok(ToolCallRunResult {
                message,
                status: ToolCallStatus::Failed,
                task_completion: None,
            });
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

    let handler = context.tools.get(&tool_call.name);
    // Unified permission decision (slab-exec-policy). The descriptor is built
    // from the tool's own `describe_operation`, falling back to a name-based
    // inference. The engine is the SINGLE owner of Allow/RequireApproval/Deny —
    // this replaces the legacy per-tool `approval_request` + risk-fallback pair
    // that could disagree (the approve-then-block bug).
    let descriptor = handler
        .as_ref()
        .and_then(|handler| handler.describe_operation(&effective_args))
        .or_else(|| infer_descriptor(&tool_call.name, &effective_args, context))
        .unwrap_or_else(|| {
            slab_exec_policy::OperationDescriptor::read_only(tool_call.name.clone())
        });
    let decision = context.exec_policy.evaluate(context.thread_id, &descriptor).await;
    let approval_request = match decision {
        slab_exec_policy::ExecDecision::Allow => None,
        slab_exec_policy::ExecDecision::Deny => {
            // Hard refusal by policy: do NOT request approval — return blocked
            // output immediately so the model learns the operation is refused.
            record_json(
                context.trace,
                &context.trace_context,
                "slab-agent",
                "tool_call_blocked_by_policy",
                serde_json::json!({
                    "item_id": tool_call.id,
                    "call_id": call_id,
                    "tool_name": tool_call.name,
                    "category": descriptor.category.as_str(),
                }),
            );
            let output = "tool call blocked by permission policy".to_string();
            emit_tool_item_failed(context, tool_call, &effective_args, &output).await;
            let message = record_failed_tool_call_without_persisting_message(
                context, &call_id, tool_call, output, created_at,
            )
            .await?;
            return Ok(ToolCallRunResult {
                message,
                status: ToolCallStatus::Failed,
                task_completion: None,
            });
        }
        slab_exec_policy::ExecDecision::RequireApproval => Some(ToolApprovalRequest {
            descriptor: descriptor.clone(),
            display: descriptor.subject.clone(),
        }),
    };
    let initial_status =
        if approval_request.is_some() { ToolCallStatus::Pending } else { ToolCallStatus::Running };
    let mut tool_state = ToolCallStateMachine::new(initial_status);
    insert_tool_call_record(context, &call_id, tool_call, tool_state.status(), created_at).await;
    let workspace_root = workspace_root_of(context);
    emit_item_started(
        context,
        tool_turn_item(tool_call, &effective_args, "running", None, workspace_root.as_deref()),
    )
    .await;

    let (tool_output, call_status) = run_tool_with_optional_approval(ToolRunContext {
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

    // A successful `task.complete` carries the structured completion payload in
    // its metadata; surface it so the turn loop can emit the final answer.
    let task_completion =
        if tool_call.name == TASK_COMPLETE_TOOL_NAME && call_status == ToolCallStatus::Completed {
            parse_task_completion(tool_output.metadata.as_ref())
        } else {
            None
        };

    let mut content = tool_output.content;
    info!(
        thread_id = context.thread_id,
        turn_index = context.turn_index,
        item_id = %tool_call.id,
        call_id = %call_id,
        tool_name = %tool_call.name,
        status = ?call_status,
        output_len = content.len(),
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
            "output": content,
        }),
    );
    append_hook_observations(&mut content, pre_observations);

    let post_event = HookEvent::OnToolEnd {
        thread_id: context.thread_id.to_owned(),
        session_id: context.session_id.to_owned(),
        turn_index: context.turn_index,
        messages: messages.to_vec(),
        call_id: call_id.clone(),
        tool_name: tool_call.name.clone(),
        arguments: effective_args.clone(),
        output: content.clone(),
        status: call_status,
    };
    let post_effects = dispatch_registered_hooks(context.hooks, &post_event).await;
    append_hook_observations(&mut content, post_effects.observations);

    let item_status =
        if matches!(call_status, ToolCallStatus::Completed) { "completed" } else { "failed" };
    emit_item_completed(
        context,
        tool_turn_item(
            tool_call,
            &effective_args,
            item_status,
            Some(&content),
            workspace_root.as_deref(),
        ),
    )
    .await;

    update_tool_call_record(context, &call_id, Some(&content), call_status).await;
    let message = crate::turn_tool_record::tool_message(tool_call, content);

    Ok(ToolCallRunResult { message, status: call_status, task_completion })
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
) -> Result<(ToolOutput, ToolCallStatus), AgentError> {
    let Some(ref request) = run.approval_request else {
        return run_tool_without_approval(&run).await;
    };
    emit_approval_request(&run).await;

    record_json(
        run.context.trace,
        &run.context.trace_context,
        "slab-agent",
        "tool_call_approval_required",
        serde_json::json!({
            "item_id": run.tool_call.id,
            "call_id": run.call_id,
            "tool_name": run.tool_call.name,
            "command": &request.display,
            "category": request.descriptor.category.as_str(),
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
            &request.descriptor,
            Some(run.risk.clone()),
        ) => decision,
        _ = run.context.cancellation.cancelled() => return Err(AgentError::Interrupted),
    };

    match decision {
        ApprovalDecision::Approved(scope) => {
            emit_approval_resolved(&run, true).await;
            // Persist the user's scope as a rule (no-op for RunOnce/Deny) so
            // future identical operations skip the prompt.
            run.context
                .exec_policy
                .remember(run.context.thread_id, &request.descriptor, scope)
                .await;
            if run.context.cancellation.is_cancelled() {
                return Err(AgentError::Interrupted);
            }
            let running_status = run.tool_state.transition(ToolCallStatus::Running)?;
            update_tool_call_status(run.context, run.call_id, running_status).await;
            emit_tool_execution_started(&run).await;
            Ok(tokio::select! {
                result = execute_tool_call(
                    run.call_id,
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
            Ok((
                ToolOutput {
                    content: "tool call rejected by approval policy".to_string(),
                    metadata: None,
                },
                ToolCallStatus::Failed,
            ))
        }
    }
}

async fn run_tool_without_approval(
    run: &ToolRunContext<'_, '_>,
) -> Result<(ToolOutput, ToolCallStatus), AgentError> {
    if run.context.cancellation.is_cancelled() {
        return Err(AgentError::Interrupted);
    }
    emit_tool_execution_started(run).await;
    Ok(tokio::select! {
        result = execute_tool_call(
            run.call_id,
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
    call_id: &str,
    tool_name: &str,
    handler: Option<Arc<dyn ToolHandler>>,
    ctx: &ToolContext,
    arguments: &serde_json::Value,
) -> (ToolOutput, ToolCallStatus) {
    let started_at = Instant::now();
    let result = if let Some(handler) = handler {
        match handler.execute(ctx, arguments).await {
            Ok(output) => (output, ToolCallStatus::Completed),
            Err(error) => {
                warn!(tool = handler.name(), error = %error, "tool execution failed");
                (ToolOutput { content: error.to_string(), metadata: None }, ToolCallStatus::Failed)
            }
        }
    } else {
        info!(tool_name = %tool_name, "agent tool call handler not found");
        warn!(tool = tool_name, "tool not found");
        (
            ToolOutput { content: format!("tool not found: {tool_name}"), metadata: None },
            ToolCallStatus::Failed,
        )
    };
    let duration = started_at.elapsed();
    let success = result.1 == ToolCallStatus::Completed;
    slab_otel::metrics::record_tool_execution(
        tool_name,
        slab_otel::gen_ai::TOOL_TYPE_FUNCTION,
        duration,
        success,
    );
    slab_otel::metrics::record_tool_count(tool_name, slab_otel::gen_ai::TOOL_TYPE_FUNCTION, 1);
    info!(
        target: "slab_otel::gen_ai",
        otel_attributes = %serde_json::json!({
            "gen_ai.operation.name": slab_otel::gen_ai::OPERATION_EXECUTE_TOOL,
            "gen_ai.tool.call.id": call_id,
            "gen_ai.tool.name": tool_name,
            "gen_ai.tool.type": slab_otel::gen_ai::TOOL_TYPE_FUNCTION,
        }),
        duration_ms = duration.as_secs_f64() * 1000.0,
        success,
        "gen_ai tool execution"
    );

    result
}

/// Infer an [`slab_exec_policy::OperationDescriptor`] for a tool that does not
/// override [`ToolHandler::describe_operation`]. Maps the tool name to a
/// category and pulls the most relevant subject (command / path / query) from
/// the arguments. Tools with their own `describe_operation` bypass this.
fn infer_descriptor(
    tool_name: &str,
    args: &serde_json::Value,
    context: &TurnExecutionContext<'_>,
) -> Option<slab_exec_policy::OperationDescriptor> {
    let workspace_root = context.thread_context.workspace.as_ref().map(|w| w.root.clone());
    let descriptor = match tool_name {
        "shell" => {
            let command = args.get("command").and_then(serde_json::Value::as_str).unwrap_or("");
            slab_exec_policy::OperationDescriptor::shell(command)
        }
        "write_file" => {
            let path = args.get("path").and_then(serde_json::Value::as_str).unwrap_or("");
            slab_exec_policy::OperationDescriptor::file_edit(path)
        }
        "apply_patch" => {
            let patch = args.get("patch").and_then(serde_json::Value::as_str).unwrap_or("");
            slab_exec_policy::OperationDescriptor::file_edit(first_path_in_patch(patch))
        }
        "web_search" => {
            let query = args.get("query").and_then(serde_json::Value::as_str).unwrap_or("");
            slab_exec_policy::OperationDescriptor::network(query)
        }
        _ => return None,
    };
    Some(descriptor.with_workspace(workspace_root))
}

/// Extract the first modified file path from a unified diff, for the file-edit
/// descriptor subject. Falls back to `"patch"` when no path can be parsed.
fn first_path_in_patch(patch: &str) -> String {
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let trimmed = rest.trim();
            // Strip the leading `b/` that git diffs use.
            let path = trimmed.strip_prefix("b/").unwrap_or(trimmed);
            let path = path.trim_matches('"');
            if !path.is_empty() && path != "/dev/null" {
                return path.to_owned();
            }
        }
    }
    "patch".to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port::ParsedToolCall;

    fn call(name: &str) -> ParsedToolCall {
        ParsedToolCall {
            id: "call-1".to_owned(),
            name: name.to_owned(),
            arguments: "{}".to_owned(),
        }
    }

    #[test]
    fn shell_maps_to_command_execution_with_workspace_cwd() {
        let item = tool_turn_item(
            &call("shell"),
            &serde_json::json!({"command": "ls -la"}),
            "running",
            None,
            Some("/ws"),
        );
        match item {
            TurnItem::CommandExecution { command, cwd, status, aggregated_output, .. } => {
                assert_eq!(command, "ls -la");
                assert_eq!(cwd, "/ws");
                assert_eq!(status, "running");
                assert!(aggregated_output.is_none());
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn fallback_tool_maps_to_command_execution_catch_all() {
        // Unknown / read-only tools fall back to CommandExecution so every tool
        // call is visible on the harness timeline (bug 1: nothing was emitted).
        let item = tool_turn_item(
            &call("read_file"),
            &serde_json::json!({}),
            "completed",
            Some("file contents"),
            None,
        );
        match item {
            TurnItem::CommandExecution { command, aggregated_output, status, .. } => {
                assert_eq!(command, "read_file");
                assert_eq!(status, "completed");
                assert_eq!(aggregated_output.as_deref(), Some("file contents"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn mcp_proxy_name_maps_to_mcp_tool_call() {
        let item = tool_turn_item(
            &call("mcp__server_label__search"),
            &serde_json::json!({"q": "x"}),
            "running",
            None,
            None,
        );
        match item {
            TurnItem::McpToolCall { server, tool, arguments, .. } => {
                assert_eq!(server, "server_label");
                assert_eq!(tool, "search");
                assert_eq!(arguments["q"], "x");
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn web_search_maps_query() {
        let item = tool_turn_item(
            &call("web_search"),
            &serde_json::json!({"query": "rust async"}),
            "running",
            None,
            None,
        );
        match item {
            TurnItem::WebSearch { query, .. } => assert_eq!(query, "rust async"),
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn apply_patch_maps_first_path_into_file_change() {
        let args = serde_json::json!({
            "patch": "--- a/x.rs\n+++ b/x.rs\n@@ -1 +1 @@\n-a\n+b\n"
        });
        let item = tool_turn_item(&call("apply_patch"), &args, "completed", None, None);
        match item {
            TurnItem::FileChange { changes, status, .. } => {
                assert_eq!(status, "completed");
                assert_eq!(changes[0]["path"].as_str(), Some("x.rs"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn parse_mcp_proxy_name_handles_namespaced_and_malformed() {
        assert_eq!(parse_mcp_proxy_name("mcp__srv__tool"), ("srv".to_owned(), "tool".to_owned()));
        // Fewer than two separators: cannot recover server/tool → placeholder.
        assert_eq!(
            parse_mcp_proxy_name("mcp__only"),
            ("<unknown>".to_owned(), "mcp__only".to_owned())
        );
    }
}
