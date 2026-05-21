//! SSE notification adapter for real-time agent event streaming.
//!
//! [`SseNotifyAdapter`] implements both [`AgentNotifyPort`] (for status changes
//! and turn events) and [`ApprovalPort`] (for interactive command approval).
//!
//! # Design
//!
//! - One `broadcast::Sender<TurnEvent>` per thread, stored in a [`DashMap`].
//!   Calling `subscribe_events()` returns a receiver that replays from the
//!   current tail.
//! - Pending approvals are stored as `oneshot::Sender<ApprovalDecision>` keyed
//!   by `"<thread_id>:<call_id>"`.  The HTTP approve handler must supply both
//!   the thread ID (from the URL path) and the call_id to prevent cross-thread
//!   approval.  Requests that receive no decision within
//!   [`APPROVAL_TIMEOUT_SECS`] are automatically rejected.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::port::{AgentNotifyPort, ApprovalDecision, ApprovalPort, ThreadStatus, TurnEvent};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, warn};

const CHANNEL_CAPACITY: usize = 256;

/// How long (in seconds) to wait for an operator approval before auto-rejecting.
const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// Shared state used by both the notify path and the HTTP handlers.
#[derive(Clone, Default)]
pub struct SseNotifyAdapter {
    /// Per-thread event broadcast channels.
    channels: Arc<DashMap<String, broadcast::Sender<TurnEvent>>>,
    /// Pending approval requests: "<thread_id>:<call_id>" → oneshot sender.
    approvals: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl SseNotifyAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to the event stream for `thread_id`.
    ///
    /// Creates the channel on first call.  The returned receiver will receive
    /// all events emitted *after* the subscription point.
    pub fn subscribe_events(&self, thread_id: &str) -> broadcast::Receiver<TurnEvent> {
        self.channels
            .entry(thread_id.to_owned())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Send an approval decision for a pending tool call.
    ///
    /// Both `thread_id` (from the URL path) and `call_id` must match the
    /// pending entry so that a caller cannot approve tool calls belonging to a
    /// different thread.
    ///
    /// Returns `true` if the pending approval was found and the decision was
    /// delivered; `false` if no matching pending approval exists.
    pub fn approve_call(&self, thread_id: &str, call_id: &str, approved: bool) -> bool {
        let key = approval_key(thread_id, call_id);
        if let Some((_, tx)) = self.approvals.remove(&key) {
            let decision =
                if approved { ApprovalDecision::Approved } else { ApprovalDecision::Rejected };
            tx.send(decision).is_ok()
        } else {
            false
        }
    }

    fn broadcast(&self, thread_id: &str, event: TurnEvent) {
        if let Some(tx) = self.channels.get(thread_id) {
            // Ignore send errors — they just mean all subscribers have dropped.
            let _ = tx.send(event);
        }
    }
}

fn approval_key(thread_id: &str, call_id: &str) -> String {
    format!("{thread_id}:{call_id}")
}

#[async_trait]
impl AgentNotifyPort for SseNotifyAdapter {
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus) {
        debug!(thread_id, ?status, "agent status change");
        // Emit a dedicated AgentStatus event so SSE consumers can track thread
        // lifecycle without receiving synthetic TurnCompleted / TurnFailed
        // payloads that duplicate what the turn loop already emits.
        self.broadcast(thread_id, TurnEvent::AgentStatus { status });
    }

    async fn on_turn_event(&self, thread_id: &str, event: &TurnEvent) {
        self.broadcast(thread_id, event.clone());
    }
}

#[async_trait]
impl ApprovalPort for SseNotifyAdapter {
    async fn request_approval(
        &self,
        thread_id: &str,
        call_id: &str,
        tool_name: &str,
        command: &str,
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        let key = approval_key(thread_id, call_id);
        self.approvals.insert(key.clone(), tx);

        // Notify SSE subscribers that approval is needed.
        self.broadcast(
            thread_id,
            TurnEvent::ApprovalRequired {
                call_id: call_id.to_owned(),
                tool_name: tool_name.to_owned(),
                command: command.to_owned(),
            },
        );

        // Wait for an operator decision, but auto-reject after the timeout so
        // the agent turn is never permanently blocked.
        let decision =
            tokio::time::timeout(std::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS), rx).await;

        // Always clean up the pending entry regardless of outcome.
        self.approvals.remove(&key);

        match decision {
            Ok(Ok(d)) => d,
            Ok(Err(_)) => {
                warn!(
                    call_id,
                    thread_id, "approval channel closed without a decision; auto-rejecting"
                );
                ApprovalDecision::Rejected
            }
            Err(_elapsed) => {
                warn!(
                    call_id,
                    thread_id,
                    "approval request timed out after {APPROVAL_TIMEOUT_SECS}s; auto-rejecting"
                );
                ApprovalDecision::Rejected
            }
        }
    }
}
