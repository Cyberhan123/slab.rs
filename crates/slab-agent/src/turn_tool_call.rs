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
        CommandExecutionOutputDeltaParams, CommandExecutionRequestApprovalParams, EventMsg,
        FileChangeOutputDeltaParams, ItemCompletedParams, ItemStartedParams, TurnItem,
    },
    state::ToolCallStateMachine,
    tool::{
        PlanRef, ToolApprovalRequest, ToolCallRender, ToolContext, ToolHandler, ToolOutput,
        ToolOutputObserver, ToolOutputStream,
    },
    turn::TurnExecutionContext,
    turn_tool_record::{
        persist_tool_message_record, record_failed_tool_call_without_persisting_message,
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

/// Tool name for on-demand Deferred-tool discovery. Mirrors
/// `slab_agent_tools::TOOL_SEARCH_TOOL_NAME`; duplicated here because
/// `slab-agent` cannot depend on `slab-agent-tools` (dependency direction is
/// reversed). `tool_search` is intercepted by the dispatch layer before
/// execution — see [`handle_tool_search`].
const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";

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
// `AgentEventKind`/`/responses` emits left this crate.

/// Workspace root for `CommandExecution.cwd`, or `None` when no workspace is bound.
fn workspace_root_of(context: &TurnExecutionContext<'_>) -> Option<String> {
    context.thread_context.workspace.as_ref().map(|w| w.root.to_string_lossy().into_owned())
}

/// Resolve a tool call to its harness [`TurnItem`].
///
/// Tools own their render via [`ToolHandler::render_turn_item`]; this looks up
/// the handler and delegates, falling back to the generic
/// [`default_tool_turn_item`] for tools not in the registry. `status` is
/// `"running"` for `ItemStarted`, `"completed"`/`"failed"` for `ItemCompleted`;
/// `output` is the tool result text (filled only on completion). The item id is
/// the provider-assigned `tool_call.id`.
#[allow(clippy::too_many_arguments)]
fn render_tool_call_item(
    handler: Option<&dyn ToolHandler>,
    tool_call: &ParsedToolCall,
    args: &serde_json::Value,
    status: &str,
    output: Option<&str>,
    workspace_root: Option<&str>,
    exit_code: Option<i64>,
    duration_ms: Option<u64>,
) -> TurnItem {
    let render = ToolCallRender {
        call: tool_call,
        args,
        status,
        output,
        workspace_root,
        exit_code,
        duration_ms,
    };
    match handler {
        Some(handler) => handler.render_turn_item(&render),
        None => crate::tool::default_tool_turn_item(&render),
    }
}

/// Handle a `tool_search` call: match the query against Deferred tool specs,
/// inject the hits into the per-thread discovery state, and return the matched
/// specs (schema-compacted) to the model.
///
/// Bypasses hooks/risk/approval (read-only registry query). Emits the standard
/// ItemStarted/ItemCompleted pair so the call is visible on the harness
/// timeline, and records the result as a tool message so it reaches the LLM.
async fn handle_tool_search(
    context: &TurnExecutionContext<'_>,
    tool_call: &ParsedToolCall,
    args: &serde_json::Value,
    _created_at: &str,
) -> Result<ToolCallRunResult, AgentError> {
    let query = args.get("query").and_then(serde_json::Value::as_str).unwrap_or("");
    let namespace = args.get("namespace").and_then(serde_json::Value::as_str);
    let q = query.to_ascii_lowercase();

    let matched: Vec<_> = context
        .tools
        .deferred_tool_specs()
        .into_iter()
        .filter(|spec| {
            if namespace.is_some_and(|ns| {
                crate::tool::ToolName::parse_wire(&spec.name).namespace.as_str() != ns
            }) {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            spec.name.to_ascii_lowercase().contains(&q)
                || spec.description.to_ascii_lowercase().contains(&q)
        })
        .collect();

    // Inject every hit so it becomes visible/callable on subsequent turns.
    for spec in &matched {
        context.tool_discovery.inject(&spec.name);
    }

    let summarized: Vec<serde_json::Value> = matched
        .iter()
        .map(|spec| {
            serde_json::json!({
                "name": spec.name,
                "description": spec.description,
                "parameters": crate::tool_schema::process_tool_schema(&spec.parameters_schema),
            })
        })
        .collect();
    let content = serde_json::to_string(&serde_json::Value::Array(summarized))
        .unwrap_or_else(|_| "[]".to_owned());

    // Emit the standard start→complete pair. tool_search renders via its
    // handler's render_turn_item (the generic CommandExecution fallback — no
    // dedicated TurnItem variant for discovery).
    let workspace_root = workspace_root_of(context);
    let handler = context.tools.get(tool_call.name.as_str());
    emit_item_started(
        context,
        render_tool_call_item(
            handler.as_deref(),
            tool_call,
            args,
            "running",
            None,
            workspace_root.as_deref(),
            None,
            None,
        ),
    )
    .await;
    emit_item_completed(
        context,
        render_tool_call_item(
            handler.as_deref(),
            tool_call,
            args,
            "completed",
            Some(&content),
            workspace_root.as_deref(),
            None,
            None,
        ),
    )
    .await;

    let message = crate::turn_tool_record::tool_message(tool_call, content);
    Ok(ToolCallRunResult { message, status: ToolCallStatus::Completed, task_completion: None })
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

/// Emit `EventMsg::CommandExecutionOutputDelta` for a running command item.
/// Display-only: the finalized output still arrives via `item/completed`.
async fn emit_command_output_delta(context: &TurnExecutionContext<'_>, item_id: &str, delta: &str) {
    let msg = EventMsg::CommandExecutionOutputDelta(CommandExecutionOutputDeltaParams {
        thread_id: context.thread_id.to_owned(),
        turn_id: context.turn_index.to_string(),
        item_id: item_id.to_owned(),
        delta: delta.to_owned(),
    });
    context.notify.on_event_msg(context.thread_id, &msg).await;
}

/// Emit `EventMsg::FileChangeOutputDelta` for a running `apply_patch` item.
/// Each delta is a JSON line `{"path": ..., "kind": ...}` reporting a file
/// committed mid-apply; the finalized change set still arrives via
/// `item/completed`.
async fn emit_file_change_delta(context: &TurnExecutionContext<'_>, item_id: &str, delta: &str) {
    let msg = EventMsg::FileChangeOutputDelta(FileChangeOutputDeltaParams {
        thread_id: context.thread_id.to_owned(),
        turn_id: context.turn_index.to_string(),
        item_id: item_id.to_owned(),
        delta: delta.to_owned(),
    });
    context.notify.on_event_msg(context.thread_id, &msg).await;
}

/// [`ToolOutputObserver`] that funnels incremental tool output into a channel a
/// concurrent drain task forwards to `emit_command_output_delta`.
struct ChannelToolOutputObserver {
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

impl ToolOutputObserver for ChannelToolOutputObserver {
    fn on_output(&self, _stream: ToolOutputStream, delta: &str) {
        let _ = self.sender.send(delta.to_string());
    }
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
    let handler = context.tools.get(&tool_call.name);
    let started = render_tool_call_item(
        handler.as_deref(),
        tool_call,
        args,
        "running",
        None,
        workspace_root.as_deref(),
        None,
        None,
    );
    emit_item_started(context, started).await;
    let completed = render_tool_call_item(
        handler.as_deref(),
        tool_call,
        args,
        "failed",
        Some(output),
        workspace_root.as_deref(),
        None,
        None,
    );
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

    // `tool_search` is a read-only meta-op that discovers Deferred tools and
    // injects them into the per-thread discovery state. Intercept it BEFORE
    // hooks/risk/approval: it needs registry + discovery access that live in the
    // dispatch layer (not on `ToolContext`), and a registry query must not be
    // approval-gated. Uses pre-hook `parsed_args` (the hook may ModifyArgs for
    // real tools, but tool_search bypasses hooks entirely).
    if tool_call.name == TOOL_SEARCH_TOOL_NAME {
        return handle_tool_search(context, tool_call, &parsed_args, created_at).await;
    }

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
    let workspace_root = workspace_root_of(context);
    emit_item_started(
        context,
        render_tool_call_item(
            handler.as_deref(),
            tool_call,
            &effective_args,
            "running",
            None,
            workspace_root.as_deref(),
            None,
            None,
        ),
    )
    .await;

    // Stream incremental command output (display-only) while the tool runs. A
    // channel-backed observer on the tool context forwards each delta to the
    // harness; the finalized result still arrives via `item/completed` below.
    let item_id = tool_call.id.clone();
    let (delta_tx, mut delta_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let started = Instant::now();
    let run = async {
        // The streaming context (and the sender inside it) is dropped at the end
        // of this block, which closes the channel so the drain below terminates.
        let mut streaming_context = tool_context.clone();
        streaming_context.output =
            Some(Arc::new(ChannelToolOutputObserver { sender: delta_tx })
                as Arc<dyn ToolOutputObserver>);
        run_tool_with_optional_approval(ToolRunContext {
            context,
            call_id: &call_id,
            tool_call,
            tool_context: &streaming_context,
            effective_args: &effective_args,
            effective_arguments: &effective_arguments,
            risk: &risk,
            handler,
            approval_request,
            tool_state: &mut tool_state,
        })
        .await
    };
    let drain = async {
        while let Some(delta) = delta_rx.recv().await {
            if tool_call.name == "apply_patch" {
                emit_file_change_delta(context, &item_id, &delta).await;
            } else {
                emit_command_output_delta(context, &item_id, &delta).await;
            }
        }
    };
    let (run_result, ()) = tokio::join!(run, drain);
    let (tool_output, call_status) = run_result?;
    let duration_ms = started.elapsed().as_millis() as u64;
    // Best-effort: surface the shell exit code on the completed item.
    let shell_exit_code = if tool_call.name == "shell" {
        serde_json::from_str::<serde_json::Value>(&tool_output.content)
            .ok()
            .and_then(|v| v.get("exit_code").and_then(|c| c.as_i64()))
    } else {
        None
    };
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
        render_tool_call_item(
            context.tools.get(&tool_call.name).as_deref(),
            tool_call,
            &effective_args,
            item_status,
            Some(&content),
            workspace_root.as_deref(),
            shell_exit_code,
            Some(duration_ms),
        ),
    )
    .await;

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
            run.tool_state.transition(ToolCallStatus::Running)?;
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

/// Extract the first modified file path from a patch, for the file-edit
/// descriptor subject. Recognizes the `*** Begin Patch` dialect headers first
/// and falls back to unified-diff `+++ b/path`. Returns `"patch"` when no path
/// can be parsed.
fn first_path_in_patch(patch: &str) -> String {
    for line in patch.lines() {
        let trimmed = line.trim_start();
        for header in ["*** Add File:", "*** Delete File:", "*** Update File:"] {
            if let Some(rest) = trimmed.strip_prefix(header) {
                let path = rest.trim().trim_matches('"');
                if !path.is_empty() {
                    return path.to_owned();
                }
            }
        }
        if let Some(rest) = trimmed.strip_prefix("+++ ") {
            let candidate = rest.trim();
            // Strip the leading `b/` that git diffs use.
            let path = candidate.strip_prefix("b/").unwrap_or(candidate).trim_matches('"');
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
    use crate::tool::{ToolCallRender, ToolContext};
    use async_trait::async_trait;

    fn call(name: &str) -> ParsedToolCall {
        ParsedToolCall {
            id: "call-1".to_owned(),
            name: name.to_owned(),
            arguments: "{}".to_owned(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_of(
        handler: Option<&dyn ToolHandler>,
        name: &str,
        args: &serde_json::Value,
        status: &str,
        output: Option<&str>,
        workspace_root: Option<&str>,
        exit_code: Option<i64>,
        duration_ms: Option<u64>,
    ) -> TurnItem {
        render_tool_call_item(
            handler,
            &call(name),
            args,
            status,
            output,
            workspace_root,
            exit_code,
            duration_ms,
        )
    }

    // Unknown tool (no handler registered) → default CommandExecution, so every
    // tool call is visible on the harness timeline.
    #[test]
    fn render_no_handler_falls_back_to_command_execution() {
        let item = render_of(
            None,
            "read_file",
            &serde_json::json!({}),
            "completed",
            Some("file contents"),
            None,
            None,
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

    // A stub tool that does NOT override render_turn_item.
    struct DefaultRenderTool;

    #[async_trait]
    impl ToolHandler for DefaultRenderTool {
        fn name(&self) -> &str {
            "default_render"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            Ok(ToolOutput { content: String::new(), metadata: None })
        }
    }

    #[test]
    fn render_turn_item_default_is_command_execution() {
        let tool = DefaultRenderTool;
        let item = render_of(
            Some(&tool),
            "default_render",
            &serde_json::json!({}),
            "running",
            None,
            None,
            None,
            None,
        );
        match item {
            TurnItem::CommandExecution { command, cwd, status, aggregated_output, .. } => {
                assert_eq!(command, "default_render");
                assert_eq!(cwd, "");
                assert_eq!(status, "running");
                assert!(aggregated_output.is_none());
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    // A stub tool that DOES override render_turn_item — verifies the dispatcher
    // delegates to the tool's own render instead of the default.
    struct CustomRenderTool;

    #[async_trait]
    impl ToolHandler for CustomRenderTool {
        fn name(&self) -> &str {
            "custom"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({})
        }
        fn render_turn_item(&self, render: &ToolCallRender<'_>) -> TurnItem {
            TurnItem::AgentMessage {
                id: render.call.id.clone(),
                text: format!("custom:{}", render.call.name),
            }
        }
        async fn execute(
            &self,
            _ctx: &ToolContext,
            _arguments: &serde_json::Value,
        ) -> Result<ToolOutput, crate::error::AgentError> {
            Ok(ToolOutput { content: String::new(), metadata: None })
        }
    }

    #[test]
    fn render_turn_item_delegates_to_handler_override() {
        let tool = CustomRenderTool;
        let item = render_of(
            Some(&tool),
            "custom",
            &serde_json::json!({}),
            "completed",
            None,
            None,
            None,
            None,
        );
        match item {
            TurnItem::AgentMessage { text, .. } => assert_eq!(text, "custom:custom"),
            other => panic!("unexpected item: {other:?}"),
        }
    }
}
