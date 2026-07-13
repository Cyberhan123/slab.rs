//! Agent event hub for real-time agent event streaming.
//!
//! [`AgentEventHub`] implements both [`AgentNotifyPort`] (for status changes and
//! harness-protocol event messages) and [`ApprovalPort`] (for interactive
//! command approval).
//!
//! # Design
//!
//! - One replaying event channel per thread, stored in a [`DashMap`]. Calling
//!   `subscribe_event_msgs()` returns recent events plus a live receiver. The
//!   channel carries slab-agent's harness protocol [`EventMsg`] (turn lifecycle
//!   / text / reasoning / tool items).
//! - Pending approvals are stored as `oneshot::Sender<ApprovalDecision>` keyed
//!   by `"<thread_id>:<call_id>"`. The HTTP approve handler must supply both
//!   the thread ID (from the URL path) and the call_id to prevent cross-thread
//!   approval. Requests that receive no decision within
//!   [`APPROVAL_TIMEOUT_SECS`] are automatically rejected.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::{
    ToolRiskAssessment,
    port::{AgentNotifyPort, ApprovalDecision, ApprovalPort, ThreadStatus},
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

/// Replay plus live receiver for the harness-protocol event stream.
///
/// Carries the slab-agent harness protocol [`EventMsg`] (turn lifecycle / text /
/// reasoning / tool items) consumed by the harness WS fan-out and turn-item
/// persistence.
pub struct AgentEventMsgSubscription {
    pub replay: Vec<AgentEventMsgEnvelope>,
    pub receiver: broadcast::Receiver<AgentEventMsgEnvelope>,
}

/// Envelope for a harness-protocol [`EventMsg`]. `id` is the per-thread
/// monotonic counter used for replay ordering.
#[derive(Clone)]
pub struct AgentEventMsgEnvelope {
    pub id: u64,
    pub msg: EventMsg,
}

#[derive(Clone)]
struct EventChannel {
    msg_sender: broadcast::Sender<AgentEventMsgEnvelope>,
    state: Arc<Mutex<EventChannelState>>,
}

#[derive(Default)]
struct EventChannelState {
    next_id: u64,
    msg_history: Vec<AgentEventMsgEnvelope>,
}

impl EventChannel {
    fn new() -> Self {
        let (msg_sender, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { msg_sender, state: Arc::new(Mutex::new(EventChannelState::default())) }
    }

    fn subscribe_msgs(&self) -> AgentEventMsgSubscription {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let receiver = self.msg_sender.subscribe();
        AgentEventMsgSubscription { replay: state.msg_history.clone(), receiver }
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

    /// Subscribe to the harness-protocol (`EventMsg`) stream for `thread_id`.
    ///
    /// Creates the channel on first call. The returned subscription includes
    /// recent events emitted before the subscription and all later live events.
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
    }

    async fn on_event_msg(&self, thread_id: &str, msg: &EventMsg) {
        self.broadcast_msg(thread_id, msg.clone());
    }
}

/// Notify port that fans out status changes and event messages to a list of
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
        _tool_name: &str,
        _descriptor: &slab_exec_policy::OperationDescriptor,
        _risk: Option<ToolRiskAssessment>,
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        let key = approval_key(call_id);
        self.approvals.insert(key.clone(), tx);

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
    use super::AgentEventHub;
    use slab_agent::protocol::EventMsg;

    #[test]
    fn subscribe_event_msgs_replays_messages_emitted_before_subscription() {
        let adapter = AgentEventHub::new();
        adapter.broadcast_msg(
            "thread-1",
            EventMsg::AgentMessageDelta(slab_agent::protocol::AgentMessageDeltaParams {
                thread_id: "thread-1".to_owned(),
                turn_id: "0".to_owned(),
                item_id: "item-1".to_owned(),
                delta: "done".into(),
            }),
        );

        let subscription = adapter.subscribe_event_msgs("thread-1");

        assert_eq!(subscription.replay.len(), 1);
        assert!(matches!(
            &subscription.replay[0].msg,
            EventMsg::AgentMessageDelta(params) if params.delta == "done"
        ));
    }
}
