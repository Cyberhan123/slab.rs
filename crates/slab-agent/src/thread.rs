//! Single agent thread lifecycle.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use tokio::sync::watch;
use tracing::{debug, error, info};
use uuid::Uuid;

use slab_agent_tracing::{AgentTraceContext, AgentTraceSink, record_json};
use slab_types::{ConversationMessage, ConversationMessageContent};

use tokio_util::sync::CancellationToken;

use crate::{
    compact::{CompactContext, CompactOutcome, CompactPort, CompactProgress},
    config::AgentConfig,
    error::AgentError,
    hook::{AgentHookRegistry, HookEvent, dispatch_registered_hooks},
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalPort, ExecPolicyPort, LlmPort, PlanStorePort,
        ThreadSnapshot, ThreadStatus,
    },
    protocol::{
        ContextCompactedParams, ContextCompactingParams, ErrorEvent, EventMsg, Turn,
        TurnAbortedParams, TurnCompletedParams, TurnStartedParams, TurnUsage,
    },
    repetition_guard::{RepetitionDetected, RepetitionGuard},
    risk::ToolRiskAnalyzer,
    state::ThreadStateMachine,
    tool::{AgentThreadContext, ToolDiscoveryState, ToolRouter},
    turn::{TurnExecutionContext, TurnOutcome, emit_message_appended, execute_turn},
};

// ── Harness-protocol (EventMsg) lifecycle emits ───────────────────────────────
//
// The harness turn lifecycle emits. slab-agent speaks `EventMsg` (its harness
// protocol) exclusively — the legacy `AgentEventKind`/`/responses` wire left
// this crate. These mirror what `HarnessProjection` used to derive.

fn harness_turn(id: String, status: &str) -> Turn {
    Turn { id, items: Vec::new(), status: status.to_owned(), error: None }
}

async fn emit_turn_started(notify: &Arc<dyn AgentNotifyPort>, thread_id: &str, turn_index: u32) {
    let msg = EventMsg::TurnStarted(TurnStartedParams {
        thread_id: thread_id.to_owned(),
        turn: harness_turn(turn_index.to_string(), "inProgress"),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_turn_completed(
    notify: &Arc<dyn AgentNotifyPort>,
    thread_id: &str,
    turn_index: u32,
    usage: Option<TurnUsage>,
) {
    let msg = EventMsg::TurnCompleted(TurnCompletedParams {
        thread_id: thread_id.to_owned(),
        turn: harness_turn(turn_index.to_string(), "completed"),
        usage,
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_turn_aborted(notify: &Arc<dyn AgentNotifyPort>, thread_id: &str, turn_index: u32) {
    let msg = EventMsg::TurnAborted(TurnAbortedParams {
        thread_id: thread_id.to_owned(),
        turn: harness_turn(turn_index.to_string(), "interrupted"),
    });
    notify.on_event_msg(thread_id, &msg).await;
}

async fn emit_turn_error(notify: &Arc<dyn AgentNotifyPort>, thread_id: &str, message: &str) {
    let msg = EventMsg::Error(ErrorEvent::new(message.to_owned()));
    notify.on_event_msg(thread_id, &msg).await;
}

/// [`CompactProgress`] impl that surfaces the auto-compaction lifecycle as
/// harness events. `on_compacting` emits `ContextCompacting` and arms `fired`;
/// the caller (`AgentThread::maybe_compact`) reads `fired` after `compact()`
/// returns so it can emit a terminal `ContextCompacted` even on the rare
/// summarize-fails→fallback-skips path (no dangling "compacting" indicator).
struct NotifyingCompactProgress {
    notify: Arc<dyn AgentNotifyPort>,
    thread_id: String,
    fired: Arc<AtomicBool>,
}

impl CompactProgress for NotifyingCompactProgress {
    fn on_compacting<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        let notify = self.notify.clone();
        let thread_id = self.thread_id.clone();
        let fired = self.fired.clone();
        Box::pin(async move {
            fired.store(true, Ordering::SeqCst);
            let msg = EventMsg::ContextCompacting(ContextCompactingParams {
                thread_id: thread_id.clone(),
            });
            notify.on_event_msg(&thread_id, &msg).await;
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum TerminationReason {
    MaxTurns,
    RepetitionDetected,
    BudgetExhausted,
}

impl TerminationReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MaxTurns => "max_turns_reached",
            Self::RepetitionDetected => "repetition_detected",
            Self::BudgetExhausted => "budget_exhausted",
        }
    }
}

/// A single agent conversation thread.
///
/// Created by [`crate::control::AgentControl`] and consumed by `AgentThread::run`.
pub struct AgentThread {
    /// Unique thread identifier.
    pub id: String,
    /// The chat session this thread belongs to.
    pub session_id: String,
    /// Parent thread ID for sub-agents; `None` for root agents.
    pub parent_id: Option<String>,
    /// Nesting depth (0 = root).
    pub depth: u32,
    /// Runtime configuration for this thread.
    pub config: AgentConfig,
    /// Shared state machine so the controller can also request lifecycle transitions.
    pub(crate) state: Arc<ThreadStateMachine>,
}

pub(crate) struct AgentThreadRuntime {
    pub llm: Arc<dyn LlmPort>,
    pub store: Arc<dyn AgentStorePort>,
    pub notify: Arc<dyn AgentNotifyPort>,
    pub approval: Arc<dyn ApprovalPort>,
    pub exec_policy: Arc<dyn ExecPolicyPort>,
    pub agent_registry: Arc<dyn crate::agent::AgentRegistry>,
    pub plan_store: Arc<dyn PlanStorePort>,
    pub tools: Arc<ToolRouter>,
    pub tool_discovery: ToolDiscoveryState,
    pub hooks: AgentHookRegistry,
    pub compact: Arc<dyn CompactPort>,
    pub risk: Arc<dyn ToolRiskAnalyzer>,
    pub trace: Arc<dyn AgentTraceSink>,
    pub trace_dir: Option<std::path::PathBuf>,
    pub thread_context: AgentThreadContext,
    pub cancellation: CancellationToken,
}

impl AgentThread {
    /// Create a new thread and return it together with a status [`watch::Receiver`].
    pub fn new(
        session_id: String,
        parent_id: Option<String>,
        depth: u32,
        config: AgentConfig,
    ) -> (Self, watch::Receiver<ThreadStatus>) {
        let id = Uuid::new_v4().to_string();
        Self::new_with_id(id, session_id, parent_id, depth, config)
    }

    pub(crate) fn new_with_id(
        id: String,
        session_id: String,
        parent_id: Option<String>,
        depth: u32,
        config: AgentConfig,
    ) -> (Self, watch::Receiver<ThreadStatus>) {
        let (state, status_rx) = ThreadStateMachine::new();
        let thread = Self { id, session_id, parent_id, depth, config, state };
        (thread, status_rx)
    }

    /// Subscribe to status changes for this thread.
    pub fn subscribe(&self) -> watch::Receiver<ThreadStatus> {
        self.state.subscribe()
    }

    /// Run the agent loop to completion, consuming `self`.
    ///
    /// Injects the system prompt (if configured), then loops over LLM turns
    /// until the model produces a final answer, `max_turns` is exhausted, or
    /// an error occurs.
    ///
    /// Returns the final assistant text on success.
    pub(crate) async fn run(
        self,
        mut messages: Vec<ConversationMessage>,
        runtime: AgentThreadRuntime,
        starting_turn_index: u32,
        emit_new: Option<usize>,
    ) -> Result<String, AgentError> {
        let AgentThreadRuntime {
            llm,
            store,
            notify,
            approval,
            exec_policy,
            agent_registry,
            plan_store,
            tools,
            tool_discovery,
            hooks,
            compact,
            risk,
            trace,
            trace_dir,
            thread_context,
            cancellation,
        } = runtime;
        let thread_id = self.id.clone();
        let mut trace_context =
            AgentTraceContext::new(self.session_id.clone()).with_thread(thread_id.clone());
        // INFRA-09: tag subagent events with the delegating parent's span id so
        // the parent→child tree can be reconstructed from the trace JSONL.
        if let Some(parent) = self.parent_id.clone() {
            trace_context = trace_context.with_parent_span_id(parent.clone());
            // Trace-bundle grouping: a child thread correlates back to
            // its ROOT thread's bundle (not its immediate parent). The root is
            // resolved by walking the persisted parent chain up to the ancestor
            // with no parent. depth-1 children resolve in one hop; depth-N
            // grandchild spawn chains (DelegateSubagentTool nesting, default
            // max_depth 4) resolve through the full chain so every descendant
            // groups under the SAME root bundle and remains reachable from the
            // rollout via the root thread's SessionMeta.trace_path. If the chain
            // cannot be walked (a parent snapshot is not yet persisted — a
            // diagnostic race), we fall back to the nearest ancestor id so the
            // event is still grouped deterministically rather than dropped.
            let root = resolve_root_thread_id(store.as_ref(), &parent).await.unwrap_or(parent);
            trace_context = trace_context.with_root_thread_id(root);
        } else {
            trace_context = trace_context.with_root_thread_id(thread_id.clone());
        }
        if let Some(trace_dir) = trace_dir {
            trace_context = trace_context.with_trace_dir(trace_dir);
        }
        let now = Utc::now().to_rfc3339();
        record_json(
            trace.as_ref(),
            &trace_context,
            "slab-agent",
            "thread_started",
            serde_json::json!({
                "session_id": self.session_id,
                "thread_id": thread_id,
                "parent_id": self.parent_id,
                "depth": self.depth,
                "starting_turn_index": starting_turn_index,
                "emit_new": emit_new,
                "config": self.config,
                "initial_messages": messages,
            }),
        );

        // Fail early if the config cannot be serialized — a swallowed error here
        // would silently persist an empty config_json and make debugging impossible.
        let config_json = serde_json::to_string(&self.config)
            .map_err(|e| AgentError::Internal(format!("failed to serialize agent config: {e}")))?;

        // Persist initial snapshot.
        let snapshot = ThreadSnapshot {
            id: thread_id.clone(),
            session_id: self.session_id.clone(),
            parent_id: self.parent_id.clone(),
            depth: self.depth,
            status: self.state.status(),
            role_name: None,
            config_json,
            completion_text: None,
            created_at: now.clone(),
            updated_at: now,
            archived_at: None,
        };
        if let Err(e) = store.upsert_thread(&snapshot).await {
            error!(thread_id, error = %e, "failed to persist thread snapshot");
        }

        if !cancellation.is_cancelled() {
            self.set_status(ThreadStatus::Running, &notify).await?;
            record_json(
                trace.as_ref(),
                &trace_context,
                "slab-agent",
                "thread_status",
                serde_json::json!({ "status": ThreadStatus::Running }),
            );

            // Persist the Running transition so the stored status matches the in-memory state.
            if let Err(e) =
                store.update_thread_status(&thread_id, ThreadStatus::Running, None).await
            {
                error!(thread_id, error = %e, "failed to persist running status");
            }
        }

        // Inject system prompt as the first message, if not already present.
        if starting_turn_index == 0
            && let Some(ref system_prompt) = self.config.system_prompt
            && !system_prompt.is_empty()
            && messages.first().map(|m| m.role.as_str()) != Some("system")
        {
            messages.insert(
                0,
                ConversationMessage {
                    role: "system".to_owned(),
                    content: ConversationMessageContent::Text(system_prompt.clone()),
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            );
            record_json(
                trace.as_ref(),
                &trace_context,
                "slab-agent",
                "system_prompt_injected",
                serde_json::json!({ "system_prompt": system_prompt }),
            );
        }

        let start_effects = dispatch_registered_hooks(
            &hooks,
            &HookEvent::OnAgentStart {
                thread_id: thread_id.clone(),
                session_id: self.session_id.clone(),
                parent_id: self.parent_id.clone(),
                depth: self.depth,
                config: self.config.clone(),
            },
        )
        .await;
        // Merge (replace-by-tag), never append: the init batch carries stable
        // `name` tags (slab_system / slab_environment / …), so a later run
        // refreshes volatile fragments (permission mode, environment time,
        // reasoning effort) IN PLACE instead of re-inserting a second full
        // batch that inflates the context every user turn.
        merge_injected_messages(&mut messages, start_effects.injected_messages);
        insert_injected_messages(
            &mut messages,
            hook_observation_messages(start_effects.observations),
        );

        if let Some(new_count) = emit_new {
            let start = messages.len().saturating_sub(new_count);
            for message in messages.iter().skip(start) {
                emit_message_appended(notify.as_ref(), &thread_id, starting_turn_index, message)
                    .await;
                record_json(
                    trace.as_ref(),
                    &trace_context,
                    "slab-agent",
                    "thread_message_persisted",
                    serde_json::json!({
                        "turn_index": starting_turn_index,
                        "message": message,
                    }),
                );
            }
        }

        let mut completion_text: Option<String> = None;
        let mut last_error: Option<AgentError> = None;
        let mut invalid_tool_call_retries = 0u8;
        let mut interrupted = false;
        let mut termination_reason: Option<TerminationReason> = None;
        let mut repetition_guard = RepetitionGuard::default();
        let mut consumed_tokens = 0u32;
        let mut last_turn_usage: Option<TurnUsage> = None;
        let mut reached_final_turn = false;

        'turns: for turn_offset in 0..self.config.max_turns {
            if cancellation.is_cancelled() {
                interrupted = true;
                break 'turns;
            }
            let turn_index = starting_turn_index + turn_offset;
            debug!(thread_id, turn_index, "starting turn");
            emit_turn_started(&notify, &thread_id, turn_index).await;
            let turn_trace_context = trace_context.clone().with_turn(turn_index);
            self.maybe_compact(
                compact.as_ref(),
                &mut messages,
                trace.as_ref(),
                &turn_trace_context,
                &notify,
            )
            .await;
            match execute_turn(
                TurnExecutionContext {
                    thread_id: &thread_id,
                    session_id: &self.session_id,
                    turn_index,
                    depth: self.depth,
                    config: &self.config,
                    llm: llm.as_ref(),
                    tools: tools.as_ref(),
                    tool_discovery: &tool_discovery,
                    notify: notify.as_ref(),
                    approval: approval.as_ref(),
                    exec_policy: exec_policy.as_ref(),
                    agent_registry: agent_registry.as_ref(),
                    plan_store: Arc::clone(&plan_store),
                    hooks: &hooks,
                    risk: risk.as_ref(),
                    trace: trace.as_ref(),
                    trace_context: turn_trace_context,
                    cancellation: &cancellation,
                    thread_context: &thread_context,
                    consumed_tokens,
                },
                &mut messages,
            )
            .await
            {
                Ok(outcome) => match outcome {
                    TurnOutcome::Final { usage } => {
                        let total = usage.as_ref().map(|u| u.total_tokens).unwrap_or_default();
                        consumed_tokens = consumed_tokens.saturating_add(total);
                        last_turn_usage = usage.map(TurnUsage::from);
                        // Extract the final assistant text.
                        reached_final_turn = true;
                        completion_text = messages.iter().rev().find_map(|m| {
                            if m.role == "assistant"
                                && let ConversationMessageContent::Text(ref t) = m.content
                                && !t.is_empty()
                            {
                                return Some(t.clone());
                            }
                            None
                        });
                        break 'turns;
                    }
                    TurnOutcome::BudgetExceeded { usage } => {
                        let total = usage.as_ref().map(|u| u.total_tokens).unwrap_or_default();
                        consumed_tokens = consumed_tokens.saturating_add(total);
                        last_turn_usage = usage.map(TurnUsage::from);
                        termination_reason = Some(TerminationReason::BudgetExhausted);
                        break 'turns;
                    }
                    TurnOutcome::ToolCalls { invalid_tool_calls, signatures, usage } => {
                        let total = usage.as_ref().map(|u| u.total_tokens).unwrap_or_default();
                        consumed_tokens = consumed_tokens.saturating_add(total);
                        last_turn_usage = usage.map(TurnUsage::from);
                        if invalid_tool_calls == 0 {
                            invalid_tool_call_retries = 0;
                        } else {
                            invalid_tool_call_retries = invalid_tool_call_retries.saturating_add(1);
                            if invalid_tool_call_retries
                                > self.config.effective_invalid_tool_call_retries()
                            {
                                last_error = Some(AgentError::Internal(format!(
                                    "invalid tool call retry budget exceeded after {invalid_tool_call_retries} invalid responses"
                                )));
                                break 'turns;
                            }
                        }
                        if let Some(detected) = repetition_guard.observe(&signatures) {
                            self.record_repetition_detected(
                                trace.as_ref(),
                                &trace_context,
                                &thread_id,
                                &detected,
                            );
                            termination_reason = Some(TerminationReason::RepetitionDetected);
                            break 'turns;
                        }
                    }
                },
                Err(e) => {
                    if matches!(e, AgentError::Interrupted) {
                        interrupted = true;
                        break 'turns;
                    }
                    error!(thread_id, turn_index, error = %e, "turn failed");
                    last_error = Some(e);
                    break 'turns;
                }
            }
        }

        if interrupted {
            self.set_status(ThreadStatus::Interrupted, &notify).await?;
            record_json(
                trace.as_ref(),
                &trace_context,
                "slab-agent",
                "thread_cancelled",
                serde_json::json!({ "status": ThreadStatus::Interrupted }),
            );
            store
                .update_thread_status(&thread_id, ThreadStatus::Interrupted, Some("interrupted"))
                .await
                .ok();
            dispatch_registered_hooks(
                &hooks,
                &HookEvent::OnAgentEnd {
                    thread_id: thread_id.clone(),
                    session_id: self.session_id.clone(),
                    status: ThreadStatus::Interrupted,
                    error: None,
                },
            )
            .await;
            return Ok(String::new());
        }

        if let Some(err) = last_error {
            emit_turn_error(&notify, &thread_id, &err.to_string()).await;
            self.set_status(ThreadStatus::Errored, &notify).await?;
            record_json(
                trace.as_ref(),
                &trace_context,
                "slab-agent",
                "thread_failed",
                serde_json::json!({
                    "status": ThreadStatus::Errored,
                    "error": err.to_string(),
                }),
            );
            store
                .update_thread_status(&thread_id, ThreadStatus::Errored, Some(&err.to_string()))
                .await
                .ok();
            dispatch_registered_hooks(
                &hooks,
                &HookEvent::OnAgentEnd {
                    thread_id: thread_id.clone(),
                    session_id: self.session_id.clone(),
                    status: ThreadStatus::Errored,
                    error: Some(err.to_string()),
                },
            )
            .await;
            return Err(err);
        }

        if !reached_final_turn {
            let termination_reason = termination_reason.unwrap_or(TerminationReason::MaxTurns);
            let reason = termination_reason.as_str();
            emit_turn_aborted(&notify, &thread_id, starting_turn_index).await;
            self.set_status(ThreadStatus::Interrupted, &notify).await?;
            record_json(
                trace.as_ref(),
                &trace_context,
                "slab-agent",
                termination_reason.trace_event_name(),
                serde_json::json!({
                    "status": ThreadStatus::Interrupted,
                    "reason": reason,
                    "max_turns": self.config.max_turns,
                    "consumed_tokens": consumed_tokens,
                    "token_budget": self.config.token_budget,
                }),
            );
            store
                .update_thread_status(&thread_id, ThreadStatus::Interrupted, Some(reason))
                .await
                .ok();
            dispatch_registered_hooks(
                &hooks,
                &HookEvent::OnAgentEnd {
                    thread_id: thread_id.clone(),
                    session_id: self.session_id.clone(),
                    status: ThreadStatus::Interrupted,
                    error: None,
                },
            )
            .await;
            return Ok(String::new());
        }

        info!(thread_id, "thread completed");
        emit_turn_completed(&notify, &thread_id, starting_turn_index, last_turn_usage.clone())
            .await;
        self.set_status(ThreadStatus::Completed, &notify).await?;
        record_json(
            trace.as_ref(),
            &trace_context,
            "slab-agent",
            "thread_completed",
            serde_json::json!({
                "status": ThreadStatus::Completed,
                "completion_text": completion_text,
                "consumed_tokens": consumed_tokens,
                "max_turns": self.config.max_turns,
                "token_budget": self.config.token_budget,
                "parent_span_id": trace_context.parent_span_id,
            }),
        );
        store
            .update_thread_status(&thread_id, ThreadStatus::Completed, completion_text.as_deref())
            .await
            .ok();
        dispatch_registered_hooks(
            &hooks,
            &HookEvent::OnAgentEnd {
                thread_id: thread_id.clone(),
                session_id: self.session_id.clone(),
                status: ThreadStatus::Completed,
                error: None,
            },
        )
        .await;

        Ok(completion_text.unwrap_or_default())
    }

    async fn set_status(
        &self,
        status: ThreadStatus,
        notify: &Arc<dyn AgentNotifyPort>,
    ) -> Result<(), AgentError> {
        self.state.transition(status)?;
        notify.on_status_change(&self.id, status).await;
        Ok(())
    }

    async fn maybe_compact(
        &self,
        compact: &dyn CompactPort,
        messages: &mut Vec<ConversationMessage>,
        trace: &dyn AgentTraceSink,
        trace_context: &AgentTraceContext,
        notify: &Arc<dyn AgentNotifyPort>,
    ) {
        let input_tokens = compact.estimate_tokens(messages);
        let threshold_tokens = compact.threshold_tokens();
        record_json(
            trace,
            trace_context,
            "slab-agent",
            "context_compaction_policy",
            serde_json::json!({
                "policy": compact.policy_name(),
                "input_tokens": input_tokens,
                "threshold_tokens": threshold_tokens,
            }),
        );

        // The threshold gate lives inside each `CompactPort` implementation
        // (context-length-aware policies need the model id; pure-local ones use
        // their fixed threshold). Auto-compaction from the turn loop never
        // forces — manual `/compact` sets `force` at its own call site. The
        // progress callback fires (emitting `ContextCompacting`) only once a
        // summarization actually begins, after every skip gate has passed.
        let fired = Arc::new(AtomicBool::new(false));
        let ctx = CompactContext {
            model_id: &self.config.model,
            summary_instructions: None,
            force: false,
            // The host policy self-queries the scheduler (slab-agent stays
            // pure); the hint stays `None` on the turn-loop path.
            memory_pressure_hint: None,
            progress: Some(Arc::new(NotifyingCompactProgress {
                notify: notify.clone(),
                thread_id: self.id.clone(),
                fired: fired.clone(),
            })),
        };
        match compact.compact(messages, &ctx).await {
            Ok(CompactOutcome::Replaced {
                messages: compacted,
                output_tokens,
                replaced_messages,
            }) => {
                *messages = compacted;
                record_json(
                    trace,
                    trace_context,
                    "slab-agent",
                    "context_compaction_completed",
                    serde_json::json!({
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "replaced_messages": replaced_messages,
                        "messages": messages,
                    }),
                );
                notify
                    .on_event_msg(
                        &self.id,
                        &EventMsg::ContextCompacted(ContextCompactedParams {
                            thread_id: self.id.clone(),
                            status: Some("compacted".to_owned()),
                            removed_messages: Some(replaced_messages as u32),
                            output_tokens: Some(output_tokens as u32),
                        }),
                    )
                    .await;
            }
            Ok(CompactOutcome::Skipped { reason }) => {
                record_json(
                    trace,
                    trace_context,
                    "slab-agent",
                    "context_compaction_skipped",
                    serde_json::json!({
                        "input_tokens": input_tokens,
                        "threshold_tokens": threshold_tokens,
                        "reason": reason,
                    }),
                );
                // Clear a previously-shown "compacting" indicator on the rare
                // path where a summarization started but ultimately skipped.
                Self::emit_compacted_skipped(notify, &self.id, fired).await;
            }
            Err(error) => {
                record_json(
                    trace,
                    trace_context,
                    "slab-agent",
                    "context_compaction_skipped",
                    serde_json::json!({
                        "input_tokens": input_tokens,
                        "threshold_tokens": threshold_tokens,
                        "reason": error.to_string(),
                    }),
                );
                Self::emit_compacted_skipped(notify, &self.id, fired).await;
            }
        }
    }

    /// Emit a terminal `ContextCompacted { status: "skipped" }` so the client
    /// clears its in-progress indicator — but only if `ContextCompacting` was
    /// actually emitted (the progress callback ran).
    async fn emit_compacted_skipped(
        notify: &Arc<dyn AgentNotifyPort>,
        thread_id: &str,
        fired: Arc<AtomicBool>,
    ) {
        if !fired.load(Ordering::SeqCst) {
            return;
        }
        notify
            .on_event_msg(
                thread_id,
                &EventMsg::ContextCompacted(ContextCompactedParams {
                    thread_id: thread_id.to_owned(),
                    status: Some("skipped".to_owned()),
                    removed_messages: None,
                    output_tokens: None,
                }),
            )
            .await;
    }

    fn record_repetition_detected(
        &self,
        trace: &dyn AgentTraceSink,
        trace_context: &AgentTraceContext,
        thread_id: &str,
        detected: &RepetitionDetected,
    ) {
        record_json(
            trace,
            trace_context,
            "slab-agent",
            "loop_detected",
            serde_json::json!({
                "thread_id": thread_id,
                "signature_hash": detected.signature.signature_hash(),
                "hit_count": detected.hit_count,
            }),
        );
    }
}

/// Resolve the ROOT thread id for a non-root thread by walking the persisted
/// parent chain up to the ancestor with no `parent_id` (the root).
///
/// Used by [`AgentThread::run`] to stamp `root_thread_id` on the trace context
/// so every descendant of a root (depth-1 children AND depth>=2 grandchildren
/// produced by nested `DelegateSubagentTool` delegation, bounded by
/// `max_depth`) groups into the SAME root bundle — keeping the grandchild
/// bundle reachable from the rollout via the root thread's
/// `SessionMeta.trace_path` (`build_session_meta` only stamps `trace_path` on
/// the true root).
///
/// Returns `None` when the chain cannot be walked (a parent snapshot is not yet
/// persisted, e.g. a diagnostic race right after spawn); the caller then falls
/// back to the nearest ancestor id so the event is still grouped
/// deterministically rather than dropped. Bounded by the spawn `max_depth`,
/// so at most `max_depth` store lookups.
pub(crate) async fn resolve_root_thread_id(
    store: &dyn AgentStorePort,
    parent_id: &str,
) -> Option<String> {
    let mut current = parent_id.to_owned();
    // Bounded by max_depth; guard against a malformed/cyclic chain defensively.
    for _ in 0..crate::control::MAX_SPAWN_DEPTH_GUARD {
        let snapshot = store.get_thread(&current).await.ok()??;
        match snapshot.parent_id {
            Some(grandparent) => current = grandparent,
            None => return Some(snapshot.id),
        }
    }
    None
}

impl TerminationReason {
    const fn trace_event_name(self) -> &'static str {
        match self {
            Self::MaxTurns => "thread_max_turns_reached",
            Self::RepetitionDetected => "thread_repetition_detected",
            Self::BudgetExhausted => "thread_token_budget_exhausted",
        }
    }
}

fn insert_injected_messages(
    messages: &mut Vec<ConversationMessage>,
    injected: Vec<ConversationMessage>,
) {
    if injected.is_empty() {
        return;
    }
    let insert_at =
        messages.iter().position(|message| message.role != "system").unwrap_or(messages.len());
    for (offset, message) in injected.into_iter().enumerate() {
        messages.insert(insert_at + offset, message);
    }
}

/// Replace-by-tag merge of an injected init-context batch into the message
/// history — the OnAgentStart path's replacement for
/// [`insert_injected_messages`] (which APPENDS a fresh batch on every run and
/// inflated the context once per user turn).
///
/// Semantics:
/// 1. Every message whose `name` matches one of the injected batch's names is
///    removed (the whole tag family — e.g. multiple `slab_agents_md` entries).
/// 2. Legacy compatibility for histories written before the tags existed:
///    untagged messages whose rendered content starts with the same XML-ish
///    wrapper prefix as an injected developer fragment
///    (`<environment_context>`, `<permissions_instructions>`, …), and an
///    untagged leading `system` whose first line equals the injected system's
///    first line, are removed too.
/// 3. The fresh batch is inserted as ONE contiguous block at the position of
///    the first removed message (index 0 when nothing was removed), so the
///    init batch always sits ahead of the user history — exactly one copy.
fn merge_injected_messages(
    messages: &mut Vec<ConversationMessage>,
    injected: Vec<ConversationMessage>,
) {
    if injected.is_empty() {
        return;
    }
    // The tag family + role-matched wrapper prefixes carried by THIS batch.
    let names: std::collections::HashSet<&str> =
        injected.iter().filter_map(|m| m.name.as_deref()).collect();
    // `(role, prefix)` pairs — a wrapper only strips UNTAGGED messages of the
    // SAME role, so a developer fragment's `<environment_context>` wrapper can
    // never strip a user turn that happens to start with the same text.
    let wrappers: Vec<(&str, String)> =
        injected.iter().filter_map(|m| wrapper_prefix(m).map(|w| (m.role.as_str(), w))).collect();
    let system_first_line =
        injected.iter().find(|m| m.role == "system").and_then(|m| first_line(&m.content));

    // Walk with index bookkeeping so the first dropped position is exact.
    let mut insert_at = None;
    let mut index = 0;
    messages.retain(|message| {
        let tagged_duplicate = message.name.as_deref().is_some_and(|n| names.contains(n));
        // A wrapper only strips UNTAGGED messages of the SAME role, so a
        // developer fragment's `<environment_context>` wrapper can never strip a
        // user turn that happens to start with the same text.
        let wrapper_duplicate = message.name.is_none()
            && wrappers.iter().any(|(role, w)| {
                message.role.as_str() == *role
                    && message.content.rendered_text().starts_with(w.as_str())
            });
        // Untagged leading system duplicated by an older build.
        let legacy_system_duplicate = message.name.is_none()
            && message.role == "system"
            && index == 0
            && system_first_line.is_some()
            && first_line(&message.content) == system_first_line;
        let drop = tagged_duplicate || wrapper_duplicate || legacy_system_duplicate;
        if drop && insert_at.is_none() {
            insert_at = Some(index);
        }
        if !drop {
            index += 1;
        }
        !drop
    });

    let at = insert_at.unwrap_or(0);
    for (offset, message) in injected.into_iter().enumerate() {
        messages.insert(at + offset, message);
    }
}

/// The leading `<wrapper>` token of a rendered fragment, when its content
/// starts with one (e.g. `<environment_context>`). Used to recognize untagged
/// legacy copies of the same fragment.
fn wrapper_prefix(message: &ConversationMessage) -> Option<String> {
    let text = message.content.rendered_text();
    let rest = text.strip_prefix('<')?;
    let end = rest.find('>')?;
    let name = &rest[..end];
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && !name.is_empty() {
        Some(format!("<{name}>"))
    } else {
        None
    }
}

fn first_line(content: &slab_types::ConversationMessageContent) -> Option<String> {
    content.rendered_text().lines().next().map(str::to_owned)
}

fn hook_observation_messages(observations: Vec<String>) -> Vec<ConversationMessage> {
    observations
        .into_iter()
        .filter(|observation| !observation.trim().is_empty())
        .map(|observation| ConversationMessage {
            role: "developer".to_owned(),
            content: ConversationMessageContent::Text(format!(
                "Local hook observation:\n{observation}"
            )),
            name: Some("slab_hook".to_owned()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use slab_types::ConversationMessageContent;

    fn msg(role: &str, text: &str, name: Option<&str>) -> ConversationMessage {
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: name.map(str::to_owned),
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn init_batch(mode_line: &str) -> Vec<ConversationMessage> {
        vec![
            msg("system", "You are Slab, an AI agent. …", Some("slab_system")),
            msg("developer", "<environment_context>\n<cwd>/w</cwd>", Some("slab_environment")),
            msg("developer", mode_line, Some("slab_permissions")),
        ]
    }

    // The context-inflation regression: merging a SECOND batch into a history
    // that already carries the first must REPLACE, not append — exactly one
    // system message, exactly one permissions fragment, and the refreshed
    // (mode-changed) content wins.
    #[test]
    fn second_merge_replaces_instead_of_appending() {
        let mut messages = init_batch("<permissions_instructions>\nmode: request_approval");
        messages.push(msg("user", "你好", None));
        messages.push(msg("assistant", "你好！", None));
        messages.push(msg("user", "再见", None));

        let mut refreshed = init_batch("<permissions_instructions>\nmode: auto_accept");
        refreshed.push(msg("developer", "<skills_instructions>\n…", Some("slab_skills")));
        merge_injected_messages(&mut messages, refreshed);

        let systems = messages.iter().filter(|m| m.role == "system").count();
        assert_eq!(systems, 1, "exactly one system message after re-merge");
        let perms: Vec<&ConversationMessage> =
            messages.iter().filter(|m| m.name.as_deref() == Some("slab_permissions")).collect();
        assert_eq!(perms.len(), 1, "permissions fragment replaced, not duplicated");
        assert!(
            perms[0].content.rendered_text().contains("auto_accept"),
            "refreshed fragment content wins"
        );
        // The batch sits as ONE contiguous block ahead of the user history.
        let first_user = messages.iter().position(|m| m.role == "user").expect("user present");
        assert!(
            !messages[..first_user].iter().any(|m| m.name.is_none()),
            "init batch is contiguous and ahead of history: {:?}",
            messages[..first_user].iter().map(|m| m.name.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            messages.iter().filter(|m| m.role == "user").count(),
            2,
            "user history untouched"
        );
    }

    // Legacy rollout files carry the init batch WITHOUT tags — the merge must
    // still recognize and replace those (wrapper-prefix + leading-system
    // first-line rules), or every resumed legacy thread would duplicate.
    #[test]
    fn merge_recleans_untagged_legacy_batch() {
        let mut messages = vec![
            msg("system", "You are Slab, an AI agent. …", None),
            msg("developer", "<environment_context>\n<cwd>/w</cwd>", None),
            msg("developer", "<permissions_instructions>\nmode: request_approval", None),
            msg("developer", "<skills_instructions>\n…", None),
            msg("user", "你好", None),
            msg("assistant", "你好！", None),
            msg("user", "再来", None),
        ];
        merge_injected_messages(
            &mut messages,
            init_batch("<permissions_instructions>\nmode: auto_accept"),
        );

        assert_eq!(
            messages.iter().filter(|m| m.role == "system").count(),
            1,
            "legacy untagged system replaced"
        );
        assert_eq!(
            messages
                .iter()
                .filter(|m| m.content.rendered_text().starts_with("<environment_context>"))
                .count(),
            1,
            "legacy untagged environment replaced"
        );
        assert_eq!(
            messages.iter().filter(|m| m.role == "user").count(),
            2,
            "user history untouched"
        );
    }

    // Fresh spawn: nothing to replace — the batch lands ahead of the user
    // messages (same placement the old insert-after-leading-system produced).
    #[test]
    fn merge_into_fresh_history_inserts_at_front() {
        let mut messages = vec![msg("user", "你好", None)];
        merge_injected_messages(&mut messages, init_batch("<permissions_instructions>\nx"));

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[3].content.rendered_text(), "你好");
    }

    // Post-compaction history (init summarized away): the merge re-inserts the
    // fresh batch ahead of the summary — exactly one copy again.
    #[test]
    fn merge_after_compaction_reinserts_single_copy() {
        let mut messages = vec![
            msg("system", "You are Slab, an AI agent. …", Some("slab_system")),
            msg("user", "summary of earlier turns", None),
        ];
        merge_injected_messages(&mut messages, init_batch("<permissions_instructions>\nx"));

        assert_eq!(
            messages.iter().filter(|m| m.role == "system").count(),
            1,
            "tagged system replaced, not duplicated"
        );
        assert_eq!(messages.len(), 4, "batch + summary");
        assert_eq!(messages[3].content.rendered_text(), "summary of earlier turns");
    }

    // A user message legitimately repeating earlier text must survive — the
    // wrapper rules only strip developer-shaped fragments, never user turns.
    #[test]
    fn merge_never_touches_user_messages() {
        let mut messages =
            vec![msg("user", "<environment_context>\nfake", None), msg("assistant", "hi", None)];
        merge_injected_messages(&mut messages, init_batch("<permissions_instructions>\nx"));
        assert_eq!(
            messages.iter().filter(|m| m.role == "user").count(),
            1,
            "user message with a wrapper-looking prefix is kept"
        );
    }
}
