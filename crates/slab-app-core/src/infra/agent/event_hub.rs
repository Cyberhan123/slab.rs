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
    protocol::EventMsg,
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

/// Replay plus live receiver for the harness-protocol event stream.
///
/// Distinct from [`AgentEventSubscription`]: that carries `AgentEventKind`
/// (the OpenAI `/responses` surface); this carries the slab-agent harness
/// protocol [`EventMsg`] (turn lifecycle / text / reasoning / tool items).
pub struct AgentEventMsgSubscription {
    pub replay: Vec<AgentEventMsgEnvelope>,
    pub receiver: broadcast::Receiver<AgentEventMsgEnvelope>,
}

#[derive(Clone)]
pub struct AgentEventEnvelope {
    pub id: u64,
    pub event: TurnEvent,
}

/// Envelope for a harness-protocol [`EventMsg`]. `id` shares the same per-thread
/// monotonic counter as [`AgentEventEnvelope::id`], so the two streams can be
/// merged deterministically by `id` if a future consumer needs total order.
#[derive(Clone)]
pub struct AgentEventMsgEnvelope {
    pub id: u64,
    pub msg: EventMsg,
}

#[derive(Clone)]
struct EventChannel {
    sender: broadcast::Sender<AgentEventEnvelope>,
    msg_sender: broadcast::Sender<AgentEventMsgEnvelope>,
    state: Arc<Mutex<EventChannelState>>,
}

#[derive(Default)]
struct EventChannelState {
    next_id: u64,
    history: Vec<AgentEventEnvelope>,
    msg_history: Vec<AgentEventMsgEnvelope>,
}

impl EventChannel {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (msg_sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { sender, msg_sender, state: Arc::new(Mutex::new(EventChannelState::default())) }
    }

    fn subscribe(&self) -> AgentEventSubscription {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.sender.subscribe();
        AgentEventSubscription { replay: state.history.clone(), receiver }
    }

    fn subscribe_msgs(&self) -> AgentEventMsgSubscription {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.msg_sender.subscribe();
        AgentEventMsgSubscription { replay: state.msg_history.clone(), receiver }
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

    fn send_msg(&self, msg: EventMsg) {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let envelope = AgentEventMsgEnvelope { id: state.next_id, msg };
        state.next_id += 1;
        if state.msg_history.len() >= CHANNEL_CAPACITY {
            state.msg_history.remove(0);
        }
        state.msg_history.push(envelope.clone());
        let _ = self.msg_sender.send(envelope);
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

    /// Subscribe to the harness-protocol (`EventMsg`) stream for `thread_id`.
    ///
    /// Sibling of [`Self::subscribe_events`]: same replay+live semantics, but
    /// for the slab-agent harness protocol surface consumed by the harness WS
    /// fan-out and turn-item persistence. Independent channel + history.
    pub fn subscribe_event_msgs(&self, thread_id: &str) -> AgentEventMsgSubscription {
        self.channel(thread_id).subscribe_msgs()
    }

    /// Send an approval decision for a pending tool call.
    ///
    /// The pending entry is matched by `call_id` alone — it is a fresh
    /// per-call UUID (globally unique), so no `thread_id` scoping is needed.
    /// Keying by `call_id` only avoids the fragile harness↔real thread-id
    /// remap that previously caused resolves to miss the pending entry.
    ///
    /// `scope` is the user's persistence choice (run-once / workspace / always
    /// / deny); it flows back to the exec-policy engine via the returned
    /// [`ApprovalDecision`].
    ///
    /// Returns `true` if the pending approval was found and the decision was
    /// delivered; `false` if no matching pending approval exists.
    pub fn approve_call(
        &self,
        thread_id: &str,
        call_id: &str,
        approved: bool,
        scope: slab_exec_policy::ApprovalScope,
    ) -> bool {
        let _ = thread_id;
        let key = approval_key(call_id);
        if let Some((_, tx)) = self.approvals.remove(&key) {
            let decision = if approved {
                ApprovalDecision::Approved(scope)
            } else {
                ApprovalDecision::Rejected
            };
            tx.send(decision).is_ok()
        } else {
            false
        }
    }

    fn broadcast(&self, thread_id: &str, event: TurnEvent) {
        self.channel(thread_id).send(event);
    }

    fn broadcast_msg(&self, thread_id: &str, msg: EventMsg) {
        self.channel(thread_id).send_msg(msg);
    }

    fn channel(&self, thread_id: &str) -> EventChannel {
        self.channels.entry(thread_id.to_owned()).or_insert_with(EventChannel::new).clone()
    }
}

fn approval_key(call_id: &str) -> String {
    format!("approval:{call_id}")
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

    async fn on_event_msg(&self, thread_id: &str, msg: &EventMsg) {
        self.broadcast_msg(thread_id, msg.clone());
    }
}

/// Notify port that fans out status changes and turn events to a list of
/// [`AgentNotifyPort`]s. Used to wire additional observers (e.g. the
/// response-persistence observer) alongside [`AgentEventHub`].
#[derive(Default)]
pub struct CompositeNotifyPort {
    inner: Vec<Arc<dyn AgentNotifyPort>>,
}

impl CompositeNotifyPort {
    pub fn new(inner: Vec<Arc<dyn AgentNotifyPort>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl AgentNotifyPort for CompositeNotifyPort {
    async fn on_status_change(&self, thread_id: &str, status: ThreadStatus) {
        for port in &self.inner {
            port.on_status_change(thread_id, status).await;
        }
    }

    async fn on_turn_event(&self, thread_id: &str, event: &TurnEvent) {
        for port in &self.inner {
            port.on_turn_event(thread_id, event).await;
        }
    }

    async fn on_event_msg(&self, thread_id: &str, msg: &EventMsg) {
        for port in &self.inner {
            port.on_event_msg(thread_id, msg).await;
        }
    }
}

#[async_trait]
impl ApprovalPort for AgentEventHub {
    async fn request_approval(
        &self,
        thread_id: &str,
        call_id: &str,
        tool_name: &str,
        descriptor: &slab_exec_policy::OperationDescriptor,
        risk: Option<ToolRiskAssessment>,
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        let key = approval_key(call_id);
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
                    command: descriptor.subject.clone(),
                    category: descriptor.category,
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
                    artifact_refs: Vec::new(),
                    reason: None,
                    phase: None,
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
