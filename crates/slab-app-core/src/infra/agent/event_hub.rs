//! Agent event hub for real-time agent event streaming.
//!
//! [`AgentEventHub`] implements both [`AgentNotifyPort`] (for status changes and
//! turn events) and [`ApprovalPort`] (for interactive command approval).
//!
//! # Design
//!
//! - One replaying event channel per thread, stored in a [`DashMap`].
//!   Calling `subscribe_events()` returns recent events plus a live receiver.
//! - Pending approvals are stored as `oneshot::Sender<ApprovalDecision>` keyed
//!   by `"<thread_id>:<call_id>"`.  The HTTP approve handler must supply both
//!   the thread ID (from the URL path) and the call_id to prevent cross-thread
//!   approval.  Requests that receive no decision within
//!   [`APPROVAL_TIMEOUT_SECS`] are automatically rejected.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::{
    AgentEventKind, ToolRiskAssessment,
    port::{AgentNotifyPort, ApprovalDecision, ApprovalPort, ThreadStatus, TurnEvent},
};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, warn};

const CHANNEL_CAPACITY: usize = 256;

/// How long (in seconds) to wait for an operator approval before auto-rejecting.
const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// Shared state used by both the notify path and the HTTP handlers.
#[derive(Clone, Default)]
pub struct AgentEventHub {
    /// Per-thread event channels with a bounded replay history.
    channels: Arc<DashMap<String, EventChannel>>,
    /// Pending approval requests: "<thread_id>:<call_id>" → oneshot sender.
    approvals: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

/// Replay plus live receiver for an agent event stream.
pub struct AgentEventSubscription {
    pub replay: Vec<AgentEventEnvelope>,
    pub receiver: broadcast::Receiver<AgentEventEnvelope>,
}

#[derive(Clone)]
pub struct AgentEventEnvelope {
    pub id: u64,
    pub event: TurnEvent,
}

#[derive(Clone)]
struct EventChannel {
    sender: broadcast::Sender<AgentEventEnvelope>,
    state: Arc<Mutex<EventChannelState>>,
}

#[derive(Default)]
struct EventChannelState {
    next_id: u64,
    history: Vec<AgentEventEnvelope>,
}

impl EventChannel {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender, state: Arc::new(Mutex::new(EventChannelState::default())) }
    }

    fn subscribe(&self) -> AgentEventSubscription {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.sender.subscribe();
        AgentEventSubscription { replay: state.history.clone(), receiver }
    }

    fn send(&self, event: TurnEvent) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let envelope = AgentEventEnvelope { id: state.next_id, event };
        state.next_id += 1;
        if state.history.len() >= CHANNEL_CAPACITY {
            state.history.remove(0);
        }
        state.history.push(envelope.clone());
        let _ = self.sender.send(envelope);
    }
}

impl AgentEventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to the event stream for `thread_id`.
    ///
    /// Creates the channel on first call.  The returned subscription includes
    /// recent events emitted before the subscription and all later live events.
    pub fn subscribe_events(&self, thread_id: &str) -> AgentEventSubscription {
        self.channel(thread_id).subscribe()
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
        self.channel(thread_id).send(event);
    }

    fn channel(&self, thread_id: &str) -> EventChannel {
        self.channels.entry(thread_id.to_owned()).or_insert_with(EventChannel::new).clone()
    }
}

fn approval_key(thread_id: &str, call_id: &str) -> String {
    format!("{thread_id}:{call_id}")
}

#[async_trait]
impl AgentNotifyPort for AgentEventHub {
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus) {
        debug!(thread_id, ?status, "agent status change");
        self.broadcast(
            thread_id,
            TurnEvent::Response { turn_index: None, event: AgentEventKind::AgentStatus { status } },
        );
    }

    async fn on_turn_event(&self, thread_id: &str, event: &TurnEvent) {
        self.broadcast(thread_id, event.clone());
    }
}

#[async_trait]
impl ApprovalPort for AgentEventHub {
    async fn request_approval(
        &self,
        thread_id: &str,
        call_id: &str,
        tool_name: &str,
        command: &str,
        risk: Option<ToolRiskAssessment>,
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        let key = approval_key(thread_id, call_id);
        self.approvals.insert(key.clone(), tx);

        // Notify SSE subscribers that approval is needed.
        self.broadcast(
            thread_id,
            TurnEvent::Response {
                turn_index: None,
                event: AgentEventKind::ResponseToolCallApprovalRequired {
                    item_id: call_id.to_owned(),
                    call_id: call_id.to_owned(),
                    tool_name: tool_name.to_owned(),
                    command: command.to_owned(),
                    risk,
                },
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

#[cfg(test)]
mod tests {
    use slab_agent::port::TurnEvent;

    use super::AgentEventHub;

    #[test]
    fn subscribe_events_replays_events_emitted_before_subscription() {
        let adapter = AgentEventHub::new();
        adapter.broadcast(
            "thread-1",
            TurnEvent::Response {
                turn_index: Some(0),
                event: slab_agent::AgentEventKind::ResponseOutputTextDone {
                    item_id: "item-1".into(),
                    output_index: 0,
                    content_index: 0,
                    text: "done".into(),
                },
            },
        );

        let subscription = adapter.subscribe_events("thread-1");

        assert_eq!(subscription.replay.len(), 1);
        assert!(matches!(
            &subscription.replay[0].event,
            TurnEvent::Response {
                event: slab_agent::AgentEventKind::ResponseOutputTextDone { text, .. },
                ..
            } if text == "done"
        ));
    }
}
