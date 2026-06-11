//! Single agent thread lifecycle.

use std::sync::Arc;

use chrono::Utc;
use tokio::sync::watch;
use tracing::{debug, error, info};
use uuid::Uuid;

use slab_agent_tracing::{AgentTraceContext, AgentTraceSink, record_json};
use slab_types::{ConversationMessage, ConversationMessageContent};

use tokio_util::sync::CancellationToken;

use crate::{
    compact::{CompactOutcome, CompactPort, compact_skipped_event},
    config::AgentConfig,
    error::AgentError,
    event::{AgentEventKind, AgentMetrics, AgentResponseRef},
    hook::{AgentHook, HookEvent, dispatch_hooks},
    port::{
        AgentNotifyPort, AgentStorePort, ApprovalPort, LlmPort, ThreadSnapshot, ThreadStatus,
        TurnEvent,
    },
    risk::ToolRiskAnalyzer,
    state::ThreadStateMachine,
    tool::ToolRouter,
    turn::{TurnExecutionContext, TurnOutcome, execute_turn, persist_thread_message},
};

/// A single agent conversation thread.
///
/// Created by [`crate::control::AgentControl`] and consumed by [`AgentThread::run`].
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
    pub tools: Arc<ToolRouter>,
    pub hooks: Arc<Vec<Arc<dyn AgentHook>>>,
    pub compact: Arc<dyn CompactPort>,
    pub risk: Arc<dyn ToolRiskAnalyzer>,
    pub trace: Arc<dyn AgentTraceSink>,
    pub trace_dir: Option<std::path::PathBuf>,
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
        persist_messages_from: Option<usize>,
    ) -> Result<String, AgentError> {
        let AgentThreadRuntime {
            llm,
            store,
            notify,
            approval,
            tools,
            hooks,
            compact,
            risk,
            trace,
            trace_dir,
            cancellation,
        } = runtime;
        let thread_id = self.id.clone();
        let mut trace_context =
            AgentTraceContext::new(self.session_id.clone()).with_thread(thread_id.clone());
        if let Some(trace_dir) = trace_dir {
            trace_context = trace_context.with_trace_dir(trace_dir);
        }
        let now = Utc::now().to_rfc3339();
        let started_at = std::time::Instant::now();
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
                "persist_messages_from": persist_messages_from,
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

        // Dispatch SessionStart hook.
        dispatch_hooks(&hooks, &HookEvent::SessionStart { thread_id: thread_id.clone() }).await;

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

        if let Some(start) = persist_messages_from {
            for message in messages.iter().skip(start) {
                persist_thread_message(store.as_ref(), &thread_id, starting_turn_index, message)
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

        'turns: for turn_offset in 0..self.config.max_turns {
            if cancellation.is_cancelled() {
                interrupted = true;
                break 'turns;
            }
            let turn_index = starting_turn_index + turn_offset;
            debug!(thread_id, turn_index, "starting turn");
            self.emit_response_event(
                &notify,
                turn_index,
                AgentEventKind::ResponseInProgress {
                    response: AgentResponseRef {
                        id: thread_id.clone(),
                        status: ThreadStatus::Running,
                    },
                },
            )
            .await;
            let turn_trace_context = trace_context.clone().with_turn(turn_index);
            self.maybe_compact(
                &notify,
                compact.as_ref(),
                &mut messages,
                turn_index,
                trace.as_ref(),
                &turn_trace_context,
            )
            .await;
            match execute_turn(
                TurnExecutionContext {
                    thread_id: &thread_id,
                    turn_index,
                    depth: self.depth,
                    config: &self.config,
                    llm: llm.as_ref(),
                    tools: tools.as_ref(),
                    store: store.as_ref(),
                    notify: notify.as_ref(),
                    approval: approval.as_ref(),
                    hooks: &hooks,
                    risk: risk.as_ref(),
                    trace: trace.as_ref(),
                    trace_context: turn_trace_context,
                    cancellation: &cancellation,
                },
                &mut messages,
            )
            .await
            {
                Ok(outcome) => match outcome {
                    TurnOutcome::Final => {
                        // Extract the final assistant text.
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
                    TurnOutcome::ToolCalls { invalid_tool_calls } => {
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

        // Dispatch Stop hook regardless of outcome.
        dispatch_hooks(&hooks, &HookEvent::Stop { thread_id: thread_id.clone() }).await;

        if interrupted {
            self.emit_response_event(
                &notify,
                starting_turn_index,
                AgentEventKind::ResponseCancelled {
                    response: AgentResponseRef {
                        id: thread_id.clone(),
                        status: ThreadStatus::Interrupted,
                    },
                    reason: "interrupted".to_owned(),
                },
            )
            .await;
            self.emit_metrics(&notify, started_at, false).await;
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
            return Ok(String::new());
        }

        if let Some(err) = last_error {
            notify
                .on_turn_event(
                    &thread_id,
                    &TurnEvent::Response {
                        turn_index: Some(starting_turn_index),
                        event: AgentEventKind::ResponseFailed {
                            response: AgentResponseRef {
                                id: thread_id.clone(),
                                status: ThreadStatus::Errored,
                            },
                            error: err.to_string(),
                        },
                    },
                )
                .await;
            self.emit_metrics(&notify, started_at, false).await;
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
            return Err(err);
        }

        info!(thread_id, "thread completed");
        self.emit_response_event(
            &notify,
            starting_turn_index,
            AgentEventKind::ResponseCompleted {
                response: AgentResponseRef {
                    id: thread_id.clone(),
                    status: ThreadStatus::Completed,
                },
            },
        )
        .await;
        self.emit_metrics(&notify, started_at, true).await;
        self.set_status(ThreadStatus::Completed, &notify).await?;
        record_json(
            trace.as_ref(),
            &trace_context,
            "slab-agent",
            "thread_completed",
            serde_json::json!({
                "status": ThreadStatus::Completed,
                "completion_text": completion_text,
            }),
        );
        store
            .update_thread_status(&thread_id, ThreadStatus::Completed, completion_text.as_deref())
            .await
            .ok();

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

    async fn emit_response_event(
        &self,
        notify: &Arc<dyn AgentNotifyPort>,
        turn_index: u32,
        event: AgentEventKind,
    ) {
        notify
            .on_turn_event(&self.id, &TurnEvent::Response { turn_index: Some(turn_index), event })
            .await;
    }

    async fn maybe_compact(
        &self,
        notify: &Arc<dyn AgentNotifyPort>,
        compact: &dyn CompactPort,
        messages: &mut Vec<ConversationMessage>,
        turn_index: u32,
        trace: &dyn AgentTraceSink,
        trace_context: &AgentTraceContext,
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
        if input_tokens < threshold_tokens {
            record_json(
                trace,
                trace_context,
                "slab-agent",
                "context_compaction_skipped",
                serde_json::json!({
                    "input_tokens": input_tokens,
                    "threshold_tokens": threshold_tokens,
                    "reason": "below threshold",
                }),
            );
            notify
                .on_turn_event(
                    &self.id,
                    &TurnEvent::Response {
                        turn_index: Some(turn_index),
                        event: compact_skipped_event(
                            input_tokens,
                            threshold_tokens,
                            "below threshold".to_owned(),
                        ),
                    },
                )
                .await;
            return;
        }

        notify
            .on_turn_event(
                &self.id,
                &TurnEvent::Response {
                    turn_index: Some(turn_index),
                    event: AgentEventKind::ResponseContextCompactStarted {
                        input_tokens,
                        threshold_tokens,
                    },
                },
            )
            .await;
        record_json(
            trace,
            trace_context,
            "slab-agent",
            "context_compaction_started",
            serde_json::json!({
                "input_tokens": input_tokens,
                "threshold_tokens": threshold_tokens,
                "message_count": messages.len(),
            }),
        );
        match compact.compact(messages).await {
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
                    .on_turn_event(
                        &self.id,
                        &TurnEvent::Response {
                            turn_index: Some(turn_index),
                            event: AgentEventKind::ResponseContextCompactCompleted {
                                input_tokens,
                                output_tokens,
                                replaced_messages,
                            },
                        },
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
                notify
                    .on_turn_event(
                        &self.id,
                        &TurnEvent::Response {
                            turn_index: Some(turn_index),
                            event: compact_skipped_event(input_tokens, threshold_tokens, reason),
                        },
                    )
                    .await;
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
                notify
                    .on_turn_event(
                        &self.id,
                        &TurnEvent::Response {
                            turn_index: Some(turn_index),
                            event: compact_skipped_event(
                                input_tokens,
                                threshold_tokens,
                                error.to_string(),
                            ),
                        },
                    )
                    .await;
            }
        }
    }

    async fn emit_metrics(
        &self,
        notify: &Arc<dyn AgentNotifyPort>,
        started_at: std::time::Instant,
        success: bool,
    ) {
        notify
            .on_turn_event(
                &self.id,
                &TurnEvent::Response {
                    turn_index: None,
                    event: AgentEventKind::ResponseMetrics {
                        metrics: AgentMetrics {
                            name: "agent_thread".to_owned(),
                            duration_ms: started_at.elapsed().as_millis(),
                            success: Some(success),
                        },
                    },
                },
            )
            .await;
    }
}
