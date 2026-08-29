//! Typed turn-phase state machine for the agent turn loop.
//!
//! The turn loop previously persisted free-form status strings
//! (`running` / `llm_completed` / `tool_calls_completed` / …) with no
//! transition guard — any site could emit any string, `started_at` was
//! restamped on every emit, and the wire used a second vocabulary
//! (`inProgress` / `completed` / `interrupted`). This module gives the turn
//! the same treatment [`crate::state`] gives threads: a typed phase enum, a
//! valid-transition lattice, and a [`TurnLifecycle`] choke point that stamps
//! `started_at` once per iteration.
//!
//! [`TurnPhase::as_str`] is the single source for both the persisted
//! `TurnStateChanged.status` and (terminal values) the restored wire
//! `Turn.status`. Terminal phases are sinks; [`TurnLifecycle::begin_iteration`]
//! is the only way out (it starts the next iteration's Sampling phase).

use std::sync::Mutex;

use crate::error::AgentError;
use crate::port::{AgentNotifyPort, ParsedToolCall};
use crate::protocol::{EventMsg, ItemCompletedParams};
use crate::tool::ToolRouter;

/// One LLM-iteration phase within a thread run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnPhase {
    /// Assembling context and streaming from the model.
    Sampling,
    /// A tool batch is in flight.
    ExecutingTools,
    /// One or more tool calls are blocked on a user approval.
    AwaitingApproval,
    /// Auto-compaction is summarizing the context.
    Compacting,
    /// Terminal: the model produced a final answer (or the `task.complete`
    /// gate passed).
    Completed,
    /// Terminal: the run was interrupted (user interrupt, max turns, budget,
    /// repetition guard).
    Interrupted,
    /// Terminal: the iteration failed (LLM error, validation error).
    Failed,
}

impl TurnPhase {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Sampling => "sampling",
            Self::ExecutingTools => "executing_tools",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Compacting => "compacting",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }

    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Interrupted | Self::Failed)
    }
}

/// Valid turn-phase transitions.
///
/// Deliberately allows `ExecutingTools ↔ AwaitingApproval` both ways: a
/// single tool batch can interleave calls that run directly with calls that
/// require approval. Terminal phases are sinks — only
/// [`TurnLifecycle::begin_iteration`] (the next iteration's reset) leaves
/// them, and it bypasses this lattice by design.
pub(crate) fn is_valid_turn_transition(from: TurnPhase, to: TurnPhase) -> bool {
    if from == to {
        return true;
    }
    match from {
        TurnPhase::Sampling => matches!(
            to,
            TurnPhase::ExecutingTools
                | TurnPhase::AwaitingApproval
                | TurnPhase::Compacting
                | TurnPhase::Completed
                | TurnPhase::Interrupted
                | TurnPhase::Failed
        ),
        TurnPhase::ExecutingTools => matches!(
            to,
            TurnPhase::Sampling
                | TurnPhase::AwaitingApproval
                | TurnPhase::Completed
                | TurnPhase::Interrupted
                | TurnPhase::Failed
        ),
        TurnPhase::AwaitingApproval => matches!(
            to,
            TurnPhase::Sampling
                | TurnPhase::ExecutingTools
                | TurnPhase::Completed
                | TurnPhase::Interrupted
                | TurnPhase::Failed
        ),
        TurnPhase::Compacting => {
            matches!(to, TurnPhase::Sampling | TurnPhase::Interrupted | TurnPhase::Failed)
        }
        TurnPhase::Completed | TurnPhase::Interrupted | TurnPhase::Failed => false,
    }
}

/// Why a thread run ended. Carried on the wire as the terminal
/// `turn/completed` / `turn/aborted` `reason` field so clients can render a
/// precise end state instead of guessing from a generic "interrupted".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminationReason {
    Completed,
    MaxTurns,
    BudgetExhausted,
    RepetitionDetected,
    Interrupted,
    Error,
}

impl TerminationReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::MaxTurns => "max_turns_reached",
            Self::BudgetExhausted => "budget_exhausted",
            Self::RepetitionDetected => "repetition_detected",
            Self::Interrupted => "interrupted",
            Self::Error => "error",
        }
    }

    /// Trace event name for the run-teardown trace record.
    pub(crate) const fn trace_event_name(self) -> &'static str {
        match self {
            Self::MaxTurns => "thread_max_turns_reached",
            Self::RepetitionDetected => "thread_repetition_detected",
            Self::BudgetExhausted => "thread_token_budget_exhausted",
            Self::Completed => "thread_completed",
            Self::Interrupted => "thread_cancelled",
            Self::Error => "thread_failed",
        }
    }
}

/// Per-run turn lifecycle: the single validation choke point for phase
/// transitions and the owner of the iteration's `started_at` (stamped ONCE
/// per iteration — previously every `TurnStateChanged` emit restamped it,
/// so a turn's rollout lines each claimed a different start time).
///
/// Interior-mutable so it can be shared as `&TurnLifecycle` through
/// [`crate::turn::TurnExecutionContext`]'s `&`-heavy style.
pub(crate) struct TurnLifecycle {
    inner: Mutex<TurnLifecycleInner>,
}

struct TurnLifecycleInner {
    turn_index: u32,
    started_at: String,
    phase: TurnPhase,
}

impl TurnLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(TurnLifecycleInner {
                turn_index: 0,
                started_at: String::new(),
                phase: TurnPhase::Sampling,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, TurnLifecycleInner> {
        // Poison-tolerant: the guarded state is plain data with no internal
        // invariants beyond the phase value itself, so recovery from a
        // poisoned lock is safe (same pattern as `event_hub`).
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Begin a new loop iteration: reset to [`TurnPhase::Sampling`] and stamp
    /// `started_at`. This is the ONLY path that leaves a terminal phase (the
    /// next iteration after a per-iteration Completed/Failed), so it bypasses
    /// the transition lattice intentionally.
    pub(crate) fn begin_iteration(&self, turn_index: u32) {
        let mut inner = self.lock();
        inner.turn_index = turn_index;
        inner.started_at = chrono::Utc::now().to_rfc3339();
        inner.phase = TurnPhase::Sampling;
    }

    /// Validate and apply a phase transition. Same-phase transitions are
    /// idempotent no-ops (a batch with two approval-gated calls legitimately
    /// "enters" `AwaitingApproval` twice).
    pub(crate) fn transition(&self, to: TurnPhase) -> Result<TurnPhase, AgentError> {
        let mut inner = self.lock();
        let from = inner.phase;
        if from == to {
            return Ok(to);
        }
        if !is_valid_turn_transition(from, to) {
            return Err(AgentError::InvalidStateTransition {
                entity: "turn",
                from: from.as_str().to_owned(),
                to: to.as_str().to_owned(),
            });
        }
        inner.phase = to;
        Ok(to)
    }

    pub(crate) fn phase(&self) -> TurnPhase {
        self.lock().phase
    }

    pub(crate) fn started_at(&self) -> String {
        self.lock().started_at.clone()
    }

    #[cfg(test)]
    pub(crate) fn turn_index(&self) -> u32 {
        self.lock().turn_index
    }
}

/// A tool item that was announced with `ItemStarted` but has not yet received
/// its `ItemCompleted`. Kept so ANY run exit path (interrupt, error, budget)
/// can still close the item — without this, an interrupted in-flight tool
/// leaves a perpetual "running" card in the UI timeline.
#[derive(Debug, Clone)]
pub(crate) struct OpenToolItem {
    pub item_id: String,
    pub turn_index: u32,
    pub tool_call: ParsedToolCall,
    pub args: serde_json::Value,
}

/// Tracks open tool items for one thread run.
#[derive(Default)]
pub(crate) struct OpenItemTracker {
    inner: Mutex<Vec<OpenToolItem>>,
}

impl OpenItemTracker {
    /// Record an `ItemStarted` whose completion is still pending.
    pub(crate) fn record_started(
        &self,
        turn_index: u32,
        tool_call: &ParsedToolCall,
        args: &serde_json::Value,
    ) {
        let mut items = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if items.iter().any(|item| item.item_id == tool_call.id) {
            return;
        }
        items.push(OpenToolItem {
            item_id: tool_call.id.clone(),
            turn_index,
            tool_call: tool_call.clone(),
            args: args.clone(),
        });
    }

    /// Record the matching `ItemCompleted` (no-op for unknown ids — the
    /// immediately-paired failure paths never register).
    pub(crate) fn record_completed(&self, item_id: &str) {
        let mut items = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        items.retain(|item| item.item_id != item_id);
    }

    pub(crate) fn open_count(&self) -> usize {
        self.inner.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    /// Emit a synthetic `ItemCompleted` for every still-open item so no
    /// `ItemStarted` is left dangling on an abnormal exit. Idempotent;
    /// clears the tracker.
    pub(crate) async fn close_all(
        &self,
        notify: &dyn AgentNotifyPort,
        thread_id: &str,
        tools: &ToolRouter,
        workspace_root: Option<&str>,
        status: &str,
        note: &str,
    ) {
        let open: Vec<OpenToolItem> =
            std::mem::take(&mut *self.inner.lock().unwrap_or_else(|p| p.into_inner()));
        for item in open {
            let handler = tools.get(&item.tool_call.name);
            let completed = crate::turn_tool_call::render_tool_call_item(
                handler.as_deref(),
                &item.tool_call,
                &item.args,
                status,
                Some(note),
                workspace_root,
                None,
                None,
            );
            let msg = EventMsg::ItemCompleted(ItemCompletedParams {
                item: completed,
                thread_id: thread_id.to_owned(),
                turn_id: item.turn_index.to_string(),
            });
            notify.on_event_msg(thread_id, &msg).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_transitions_to_every_running_phase() {
        for to in [
            TurnPhase::ExecutingTools,
            TurnPhase::AwaitingApproval,
            TurnPhase::Compacting,
            TurnPhase::Completed,
            TurnPhase::Interrupted,
            TurnPhase::Failed,
        ] {
            assert!(
                is_valid_turn_transition(TurnPhase::Sampling, to),
                "sampling → {:?} should be valid",
                to
            );
        }
    }

    #[test]
    fn terminal_phases_are_sinks() {
        for terminal in [TurnPhase::Completed, TurnPhase::Interrupted, TurnPhase::Failed] {
            for to in [
                TurnPhase::Sampling,
                TurnPhase::ExecutingTools,
                TurnPhase::AwaitingApproval,
                TurnPhase::Compacting,
            ] {
                assert!(
                    !is_valid_turn_transition(terminal, to),
                    "{terminal:?} → {to:?} must be rejected"
                );
            }
        }
    }

    #[test]
    fn tools_and_approval_interleave_both_ways() {
        assert!(is_valid_turn_transition(TurnPhase::ExecutingTools, TurnPhase::AwaitingApproval));
        assert!(is_valid_turn_transition(TurnPhase::AwaitingApproval, TurnPhase::ExecutingTools));
    }

    #[test]
    fn lifecycle_rejects_invalid_and_accepts_idempotent() {
        let lifecycle = TurnLifecycle::new();
        lifecycle.begin_iteration(3);
        assert_eq!(lifecycle.phase(), TurnPhase::Sampling);

        lifecycle.transition(TurnPhase::Failed).expect("failed");
        let err = lifecycle
            .transition(TurnPhase::Interrupted)
            .expect_err("terminal → terminal must be rejected");
        assert!(matches!(err, AgentError::InvalidStateTransition { entity: "turn", .. }));

        // Same-phase transition is an idempotent no-op.
        lifecycle.transition(TurnPhase::Failed).expect("idempotent");

        // begin_iteration is the only exit from a terminal phase.
        lifecycle.begin_iteration(4);
        assert_eq!(lifecycle.phase(), TurnPhase::Sampling);
        assert_eq!(lifecycle.turn_index(), 4);
    }

    #[test]
    fn begin_iteration_restamps_started_at_once() {
        let lifecycle = TurnLifecycle::new();
        lifecycle.begin_iteration(1);
        let first = lifecycle.started_at();
        assert_eq!(lifecycle.started_at(), first, "reads are stable");
        lifecycle.begin_iteration(2);
        assert_eq!(lifecycle.turn_index(), 2);
    }

    #[test]
    fn termination_reason_strings_are_snake_case() {
        assert_eq!(TerminationReason::MaxTurns.as_str(), "max_turns_reached");
        assert_eq!(TerminationReason::Interrupted.as_str(), "interrupted");
        assert_eq!(TerminationReason::Error.as_str(), "error");
    }
}
