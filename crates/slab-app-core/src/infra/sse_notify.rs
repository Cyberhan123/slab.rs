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
//!   by `call_id`.  The HTTP approve handler looks up and fires the sender.

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::port::{
    AgentNotifyPort, ApprovalDecision, ApprovalPort, ThreadStatus, TurnEvent,
};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, warn};

const CHANNEL_CAPACITY: usize = 256;

/// Shared state used by both the notify path and the HTTP handlers.
#[derive(Clone, Default)]
pub struct SseNotifyAdapter {
    /// Per-thread event broadcast channels.
    channels: Arc<DashMap<String, broadcast::Sender<TurnEvent>>>,
    /// Pending approval requests: call_id → oneshot sender.
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

    /// Send an approval decision for a pending `call_id`.
    ///
    /// Returns `true` if the approval channel was found and the decision was
    /// delivered; `false` if no pending approval with that `call_id` exists.
    pub fn approve_call(&self, call_id: &str, approved: bool) -> bool {
        if let Some((_, tx)) = self.approvals.remove(call_id) {
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

#[async_trait]
impl AgentNotifyPort for SseNotifyAdapter {
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus) {
        debug!(thread_id, ?status, "agent status change");
        // Convert to a TurnEvent-style notification so SSE subscribers see
        // terminal transitions even if they only consume TurnEvent.
        match status {
            ThreadStatus::Completed => {
                self.broadcast(
                    thread_id,
                    TurnEvent::TurnCompleted { text: String::new() },
                );
            }
            ThreadStatus::Errored => {
                self.broadcast(
                    thread_id,
                    TurnEvent::TurnFailed { error: "thread errored".into() },
                );
            }
            _ => {}
        }
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
        self.approvals.insert(call_id.to_owned(), tx);

        // Notify SSE subscribers that approval is needed.
        self.broadcast(
            thread_id,
            TurnEvent::ApprovalRequired {
                call_id: call_id.to_owned(),
                tool_name: tool_name.to_owned(),
                command: command.to_owned(),
            },
        );

        rx.await.map_err(|_| {
            tracing::warn!(call_id, "approval channel closed without a decision; defaulting to Rejected");
            ApprovalDecision::Rejected
        }).unwrap_or(ApprovalDecision::Rejected)
    }
}
