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
//!   `APPROVAL_TIMEOUT_SECS` are automatically rejected.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use dashmap::DashMap;
use slab_agent::{
    ToolRiskAssessment,
    port::{AgentNotifyPort, ApprovalDecision, ApprovalPort, ThreadStatus},
    protocol::EventMsg,
};
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::{debug, warn};

const CHANNEL_CAPACITY: usize = 256;

/// Returns true for the STRUCTURAL persistence-grade event subset — the
/// variants the observer maps to dedicated rollout line kinds
/// (`TurnItem` / `TurnContext::MessageAppend` / `TurnContext::TurnState` /
/// `Compacted`) ahead of the `should_persist` fallback. UI-only deltas
/// (text/reasoning deltas, approvals) are NOT structural.
///
/// Slice E.2 NOTE: this NO LONGER gates routing. `on_event_msg` routes EVERY
/// event to the dedicated unbounded persistence channel; the observer's
/// `EventPersistenceMode::should_persist` fallback decides which non-structural
/// variants (Error/Warning under Limited, deltas/approvals under Extended)
/// become `RolloutItem::EventMsg` lines. Routing only the structural subset
/// would make that fallback unreachable and silently drop Error/Warning from
/// the rollout under the default Limited mode. This helper is retained as a
/// structural classifier for test sanity assertions (e.g. Test C confirms its
/// chosen flood event is a real structural event).
#[cfg(test)]
fn is_persistence_grade(msg: &EventMsg) -> bool {
    matches!(
        msg,
        EventMsg::ItemCompleted(_)
            | EventMsg::ContextCompacting(_)
            | EventMsg::ContextCompacted(_)
            | EventMsg::ThreadStarted(_)
            | EventMsg::TurnStarted(_)
            | EventMsg::TurnCompleted(_)
            | EventMsg::TurnAborted(_)
            | EventMsg::MessageAppended(_)
            | EventMsg::TurnStateChanged(_)
    )
}

/// How long (in seconds) to wait for an operator approval before auto-rejecting.
const APPROVAL_TIMEOUT_SECS: u64 = 300;

/// Shared state used by both the notify path and the HTTP handlers.
#[derive(Clone, Default)]
pub struct AgentEventHub {
    /// Per-thread event channels with a bounded replay history.
    channels: Arc<DashMap<String, EventChannel>>,
    /// Pending approval requests: "<thread_id>:<call_id>" → oneshot sender.
    approvals: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
    /// Slice E.2 (D2): per-thread DEDICATED UNBOUNDED persistence channel.
    /// Replaces the bounded broadcast for the rollout persistence observer so
    /// a flood of persistence-grade events CANNOT `Lagged`-drop conversation
    /// data (the #1 false-green hole). The sender is registered when the
    /// observer subscribes (`persistence_subscribe`); routing pushes
    /// persistence-grade events here instead of relying on the UI broadcast.
    /// Also carries cross-turn BARRIER sentinels (FIFO-ordered after the events
    /// they must fence) — see [`AgentEventHub::persistence_barrier`].
    persistence_senders: Arc<DashMap<String, mpsc::UnboundedSender<PersistenceMessage>>>,
    /// Spawn-race replay buffer: persistence-grade events emitted BEFORE the
    /// observer subscribed are buffered here (under the per-thread lock) and
    /// drained atomically with the sender registration in `persistence_subscribe`.
    /// After subscribe, routing sends directly to the mpsc (no replay push) so
    /// the buffer does not grow unbounded for a live thread.
    persistence_replay: Arc<DashMap<String, Arc<Mutex<Vec<EventMsg>>>>>,
}

/// What travels on the dedicated persistence channel.
///
/// `Event` is a persistence-grade [`EventMsg`] the observer lands in the rollout.
/// `Barrier` is a FIFO fence: when the observer reaches it (after every prior
/// `Event`), it flushes the recorder and replies on the oneshot, proving every
/// earlier event is durable. `fork` / `compact` / `rollback` / `restore` enqueue
/// a `Barrier` before re-reading the rollout — this is the cross-turn barrier
/// (deterministic, zero-latency when the observer is caught up, no timing window).
pub(crate) enum PersistenceMessage {
    // Boxed: `EventMsg` is large (carries `TurnItem` / message vecs); without
    // boxing the `Barrier` variant (a single oneshot sender) would inflate to
    // `EventMsg`'s size (clippy::large_enum_variant).
    Event(Box<EventMsg>),
    Barrier(oneshot::Sender<()>),
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

    /// Slice E.2 (D2): route a persistence-grade event into the DEDICATED
    /// UNBOUNDED persistence channel. Called from `on_event_msg` IN ADDITION to
    /// the UI broadcast.
    ///
    /// Atomic w.r.t. `persistence_subscribe`: under the per-thread replay lock,
    /// if a sender is registered → `unbounded_send` (never blocks/Lags); else →
    /// buffer in the replay Vec (spawn-race window). `persistence_subscribe`
    /// drains the replay + registers the sender under the SAME lock, so no event
    /// can be lost (slip between drain + register) or duplicated (snapshot + mpsc).
    fn route_persistence(&self, thread_id: &str, msg: EventMsg) {
        let replay = self
            .persistence_replay
            .entry(thread_id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
            .clone();
        // Hold the replay lock for the whole routing decision so this is atomic
        // w.r.t. `persistence_subscribe` (which drains + registers the sender
        // under the same lock). An event is therefore delivered EXACTLY once:
        // either buffered in the replay snapshot (pre-subscribe) or sent on the
        // mpsc (post-subscribe), never both, never neither. The unbounded send
        // never blocks, so holding the lock across it is cheap.
        let mut guard = replay.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(sender) = self.persistence_senders.get(thread_id) {
            let _ = sender.send(PersistenceMessage::Event(Box::new(msg)));
        } else {
            // Observer not yet subscribed — buffer for the spawn-race window.
            // This branch stops firing once `persistence_subscribe` registers a
            // sender, so the buffer does NOT grow unbounded for a live thread.
            guard.push(msg);
        }
    }

    /// Slice E.2 (D2): subscribe the rollout persistence observer to the
    /// DEDICATED UNBOUNDED persistence channel for `thread_id`.
    ///
    /// Returns (replay snapshot, mpsc receiver). The replay snapshot captures
    /// persistence-grade events emitted before this call (spawn-race window);
    /// the mpsc receiver delivers all subsequent [`PersistenceMessage`]s (events
    /// AND barrier sentinels) in FIFO order with NO `Lagged` branch possible.
    ///
    /// Atomic: the replay drain + sender registration happen under the SAME
    /// per-thread replay lock that `route_persistence` takes, so an event is
    /// delivered EXACTLY once — either in the snapshot (pre-subscribe) or on the
    /// mpsc (post-subscribe), never both, never neither.
    pub(crate) fn persistence_subscribe(
        &self,
        thread_id: &str,
    ) -> (Vec<EventMsg>, mpsc::UnboundedReceiver<PersistenceMessage>) {
        let (tx, rx) = mpsc::unbounded_channel::<PersistenceMessage>();
        let replay = self
            .persistence_replay
            .entry(thread_id.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
            .clone();
        // Hold the replay lock across drain + sender registration so routing
        // cannot slip an event between the snapshot and the sender going live.
        let snapshot = {
            let mut guard = replay.lock().unwrap_or_else(|p| p.into_inner());
            self.persistence_senders.insert(thread_id.to_owned(), tx);
            std::mem::take(&mut *guard)
        };
        (snapshot, rx)
    }

    /// Slice E.2 (D2): the cross-turn barrier. Enqueue a FIFO sentinel AFTER
    /// every persistence event already emitted for `thread_id` and return a
    /// receiver that resolves once the observer has reached + flushed it — which
    /// (FIFO ordering of the unbounded mpsc) means EVERY prior event is durable.
    ///
    /// `fork` / `compact` / `rollback` / `restore` call this before re-reading
    /// the rollout. Deterministic (no timing/quiescence window) and zero-latency
    /// when the observer is caught up. Returns `None` if no observer is running
    /// for the thread (the on-disk rollout is authoritative; nothing to wait for).
    pub fn persistence_barrier(&self, thread_id: &str) -> Option<oneshot::Receiver<()>> {
        let (tx, rx) = oneshot::channel();
        let sender_ref = self.persistence_senders.get(thread_id)?;
        match sender_ref.send(PersistenceMessage::Barrier(tx)) {
            Ok(()) => Some(rx),
            Err(_) => {
                // Sender closed (observer task exited) — treat as no-op: the
                // rollout holds whatever the observer already flushed.
                drop(rx);
                None
            }
        }
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
        // Always broadcast to the UI (unchanged).
        self.broadcast_msg(thread_id, msg.clone());
        // Slice E.2 (D2): route EVERY event to the dedicated unbounded
        // persistence channel so the observer never Lag-drops conversation data.
        // The observer's `EventPersistenceMode::should_persist` fallback arm
        // decides which non-structural variants (Error/Warning under Limited,
        // deltas/approvals under Extended) become `RolloutItem::EventMsg` lines
        // — exactly the pre-slice semantics (the old observer subscribed to the
        // full broadcast and filtered by should_persist). Routing only the
        // 9 structural `is_persistence_grade` variants would make that fallback
        // unreachable for Error/Warning, silently dropping them from the rollout
        // timeline under the default Limited mode. The mpsc is unbounded ⇒ the
        // UI-delta flood that motivated D2 still cannot Lag-drop the structural
        // conversation events (the no-Lag guarantee is pinned by Test C).
        self.route_persistence(thread_id, msg.clone());
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
    use super::{AgentEventHub, is_persistence_grade};
    use slab_agent::protocol::{EventMsg, ItemCompletedParams, Turn, TurnStartedParams};

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

    // Test C (P4) — the no-Lag guarantee for the DEDICATED UNBOUNDED persistence
    // channel.
    //
    // Push 10 000 persistence-grade events WITHOUT draining the receiver (so they
    // all buffer), then drain and assert every one arrives. The unbounded mpsc
    // can never `Lagged`-drop, so this passes.
    //
    // Mutation that MUST fail: revert routing to the bounded UI broadcast
    // (`CHANNEL_CAPACITY = 256`). The first 10 000 events overflow the 256-slot
    // broadcast; `recv()` returns `Lagged(9744)` and only the last 256 envelopes
    // are recoverable → the count assertion fails (256, not 10 000). This is the
    // exact false-green hole the unbounded channel closes (a flood of items
    // between a `MessageAppended` and its `ItemCompleted` would silently drop
    // the user message under the old broadcast).
    #[tokio::test]
    async fn persistence_channel_delivers_all_events_under_flood_no_lag() {
        let hub = AgentEventHub::new();
        // Subscribe the observer FIRST so events route to the mpsc (not the
        // spawn-race replay buffer). The replay path is covered separately.
        let (_snapshot, mut rx) = hub.persistence_subscribe("t-flood");

        let make_event = |i: u32| {
            EventMsg::ItemCompleted(ItemCompletedParams {
                item: slab_agent::protocol::TurnItem::AgentMessage {
                    id: format!("m{i}"),
                    text: format!("t{i}"),
                },
                thread_id: "t-flood".to_owned(),
                turn_id: "0".to_owned(),
            })
        };
        // Sanity: the chosen event IS persistence-grade (else routing skips it
        // and the test would pass for the wrong reason — a true false-green).
        assert!(is_persistence_grade(&make_event(0)));

        const N: u32 = 10_000;
        for i in 0..N {
            // Route directly (synchronous; on_event_msg would also work but
            // route_persistence isolates the persistence path from the UI
            // broadcast for this test).
            hub.route_persistence("t-flood", make_event(i));
        }

        // Drain — every event must arrive (the unbounded mpsc never drops). The
        // hub holds its sender for life, so recv() would block forever after the
        // last event; break once we have them all instead of waiting for close.
        let mut received = 0u32;
        while received < N {
            let _msg = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
                .await
                .expect("timed out waiting for a persistence event")
                .expect("persistence channel closed before all events arrived");
            // PersistenceMessage::Event (the only variant route_persistence sends).
            assert!(
                matches!(_msg, super::PersistenceMessage::Event(_)),
                "expected an Event on the persistence channel"
            );
            received += 1;
        }
        assert_eq!(received, N, "unbounded persistence channel must deliver all {N} events");
    }

    // Spawn-race replay: persistence-grade events emitted BEFORE the observer
    // subscribes must be delivered via the replay snapshot (not lost). This is
    // the second half of the no-loss guarantee — covers the window between
    // thread creation and `ensure_rollout_persistence`.
    #[tokio::test]
    async fn persistence_replay_captures_events_before_subscribe() {
        let hub = AgentEventHub::new();
        let make_event = |i: u32| {
            EventMsg::TurnStarted(TurnStartedParams {
                thread_id: "t-race".to_owned(),
                turn: Turn { id: i.to_string(), ..Default::default() },
            })
        };
        // Emit 3 events BEFORE subscribing (no sender registered → buffered).
        for i in 0..3 {
            hub.route_persistence("t-race", make_event(i));
        }
        // Subscribe — drains the replay snapshot.
        let (snapshot, mut rx) = hub.persistence_subscribe("t-race");
        assert_eq!(snapshot.len(), 3, "spawn-race replay captured all 3 pre-subscribe events");
        // Emit one more AFTER subscribe → arrives on the mpsc.
        hub.route_persistence("t-race", make_event(3));
        let live = rx.recv().await.expect("post-subscribe event on mpsc");
        let is_turn_3 = match live {
            super::PersistenceMessage::Event(boxed) => {
                matches!(&*boxed, EventMsg::TurnStarted(p) if p.turn.id == "3")
            }
            _ => false,
        };
        assert!(is_turn_3, "expected the live turn-3 event on the mpsc");
    }
}
