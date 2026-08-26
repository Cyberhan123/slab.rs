//! Per-connection harness session state.
//!
//! [`HarnessSession`] owns the connection-level transient state — the bound
//! `session_id`, the [`HarnessService`] handle, the outbound [`Notifier`], the
//! harness-id ↔ real-id binding table, the `initialize` handshake flag, and the
//! harness-thread-id mint. It is cheap to clone (an `Arc` inside) so it can be
//! passed **by value** to typed handlers (see [`slab_jsonrpc::router`]: context
//! must be by-value for `Send + 'static` handler futures).
//!
//! State is connection-scoped: a reconnect gets a fresh session and rebuilds
//! its transient state via `thread/resume` from the persistence layer. The
//! `session_id` is only a namespace key for [`HarnessService`], never an in-memory
//! isolation boundary.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use slab_agent::protocol::{EventMsg, TurnCompletedParams};
use slab_app_core::application::agent::projection::harness::event_msg_to_notification;
use slab_app_core::context::AppState;
use slab_app_core::domain::services::HarnessService;
use slab_jsonrpc::notifier::Notifier;
use slab_proto::harness::method;
use slab_proto::harness::notification::ErrorParams;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// harness-visible thread id → real slab thread id, set once the first turn
/// materializes the thread.
#[derive(Default)]
struct ThreadBinding {
    real_id: Option<String>,
}

/// Connection-level harness state. Cheap to clone (`Arc` inside).
#[derive(Clone)]
pub(crate) struct HarnessSession {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    session_id: String,
    state: Arc<AppState>,
    service: HarnessService,
    notifier: Notifier,
    bindings: StdMutex<HashMap<String, ThreadBinding>>,
    /// Live fan-out task handles, keyed by real thread id, so a re-resume on the
    /// same connection (e.g. after `/compact` or rollback) doesn't spawn a second
    /// subscriber that would double-deliver every event. See `spawn_event_fanout`.
    fanout_tasks: StdMutex<HashMap<String, JoinHandle<()>>>,
    initialized: AtomicBool,
    next_thread_id: AtomicU64,
}

impl HarnessSession {
    pub(crate) fn new(
        session_id: String,
        state: Arc<AppState>,
        service: HarnessService,
        notifier: Notifier,
    ) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                session_id,
                state,
                service,
                notifier,
                bindings: StdMutex::new(HashMap::new()),
                fanout_tasks: StdMutex::new(HashMap::new()),
                initialized: AtomicBool::new(false),
                next_thread_id: AtomicU64::new(1),
            }),
        }
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.inner.session_id
    }

    pub(crate) fn state(&self) -> &Arc<AppState> {
        &self.inner.state
    }

    pub(crate) fn service(&self) -> &HarnessService {
        &self.inner.service
    }

    /// Surface the outbound notifier. Used by request handlers that emit
    /// notifications directly (e.g. `turn/start` emits `model/load/*` before the
    /// turn's fan-out is established).
    pub(crate) fn notifier(&self) -> &Notifier {
        &self.inner.notifier
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.inner.initialized.load(Ordering::Acquire)
    }

    pub(crate) fn mark_initialized(&self) {
        self.inner.initialized.store(true, Ordering::Release);
    }

    /// Lock the thread-binding table, recovering from a poisoned lock instead
    /// of panicking.
    fn bindings(&self) -> std::sync::MutexGuard<'_, HashMap<String, ThreadBinding>> {
        self.inner.bindings.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Resolve the real slab thread id for a harness thread id, falling back to
    /// the harness id itself (so direct/external thread ids still work).
    pub(crate) fn real_id_for(&self, harness_id: &str) -> String {
        self.bindings()
            .get(harness_id)
            .and_then(|binding| binding.real_id.clone())
            .unwrap_or_else(|| harness_id.to_owned())
    }

    /// Look up an existing real thread id without inserting a fallback.
    pub(crate) fn existing_real(&self, harness_id: &str) -> Option<String> {
        self.bindings().get(harness_id).and_then(|binding| binding.real_id.clone())
    }

    pub(crate) fn bind(&self, harness_id: &str, real_id: String) {
        self.bindings().insert(harness_id.to_owned(), ThreadBinding { real_id: Some(real_id) });
    }

    /// Insert an empty binding (for `thread/start`, which mints an id before any
    /// real thread exists).
    pub(crate) fn bind_empty(&self, harness_id: &str) {
        self.bindings().insert(harness_id.to_owned(), ThreadBinding::default());
    }

    pub(crate) fn mint_thread_id(&self) -> String {
        format!("hthread-{}", self.inner.next_thread_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Spawn a per-thread fan-out task: subscribe to the harness-protocol
    /// (`EventMsg`) stream, rewrite each event's real thread id to the
    /// harness-visible id, and push them as JSON-RPC notifications onto the
    /// session's outbound stream.
    ///
    /// slab-agent emits the harness protocol directly (`EventMsg`), so this
    /// consumes `EventMsg` only — no projection layer. The legacy
    /// `AgentEventKind` stream stays separate and feeds `/responses`.
    ///
    /// Idempotent per live real thread id: a re-resume on the same connection
    /// (e.g. `/compact` and rollback both re-resume to refresh messages) is a
    /// no-op while a fan-out task for that real thread is still running, so
    /// events are never double-delivered. A finished task (its receiver hit
    /// `Closed`) is replaced, allowing re-establishment after task death.
    pub(crate) fn spawn_event_fanout(&self, real_thread_id: String, harness_thread_id: String) {
        let mut tasks = self.inner.fanout_tasks.lock().expect("fanout task map poisoned");
        if matches!(dedupe_fanout(&tasks, &real_thread_id), FanoutDedupe::Skip) {
            return;
        }

        let service = self.inner.service.clone();
        let notifier = self.inner.notifier.clone();
        let real_key = real_thread_id.clone();
        let handle = tokio::spawn(async move {
            let subscription = service.subscribe_event_msgs(&real_thread_id);

            for envelope in &subscription.replay {
                let msg = rewrite_thread_id(envelope.msg.clone(), &harness_thread_id);
                push_event(&notifier, &harness_thread_id, msg);
            }

            let mut receiver = subscription.receiver;
            loop {
                match receiver.recv().await {
                    Ok(envelope) => {
                        let msg = rewrite_thread_id(envelope.msg, &harness_thread_id);
                        push_event(&notifier, &harness_thread_id, msg);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        notifier.notify(
                            method::ERROR,
                            &ErrorParams {
                                thread_id: Some(harness_thread_id.clone()),
                                turn_id: None,
                                item_id: None,
                                code: "stream_lagged".to_owned(),
                                message:
                                    "agent event stream lagged; some events may have been dropped"
                                        .to_owned(),
                                data: None,
                            },
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        tasks.insert(real_key, handle);
    }
}

/// Rewrite the top-level `thread_id` of an [`EventMsg`] from the real slab
/// thread id to the harness-visible thread id. Nested `TurnItem`s carry no
/// `thread_id`, so only the params' `thread_id` field is rewritten; `turn_id`
/// and item ids are preserved.
fn rewrite_thread_id(mut msg: EventMsg, harness_id: &str) -> EventMsg {
    let tid = harness_id.to_owned();
    match &mut msg {
        EventMsg::ThreadStatusChanged(p) => p.thread_id = tid.clone(),
        EventMsg::TurnStarted(p) => p.thread_id = tid.clone(),
        EventMsg::TurnCompleted(p) => p.thread_id = tid.clone(),
        EventMsg::TurnAborted(p) => p.thread_id = tid.clone(),
        EventMsg::ItemStarted(p) => p.thread_id = tid.clone(),
        EventMsg::ItemCompleted(p) => p.thread_id = tid.clone(),
        EventMsg::AgentMessageDelta(p) => p.thread_id = tid.clone(),
        EventMsg::ReasoningTextDelta(p) => p.thread_id = tid.clone(),
        EventMsg::ReasoningSummaryTextDelta(p) => p.thread_id = tid.clone(),
        EventMsg::CommandExecutionOutputDelta(p) => p.thread_id = tid.clone(),
        EventMsg::FileChangeOutputDelta(p) => p.thread_id = tid.clone(),
        EventMsg::CommandExecutionRequestApproval(p) => p.thread_id = tid.clone(),
        EventMsg::FileChangeRequestApproval(p) => p.thread_id = tid.clone(),
        EventMsg::ContextCompacting(p) => p.thread_id = tid.clone(),
        EventMsg::ContextCompacted(p) => p.thread_id = tid.clone(),
        // Error / Warning / ThreadStarted carry no thread_id; leave unchanged.
        // `EventMsg` is `#[non_exhaustive]`: future slab-agent variants with no
        // known thread_id mapping pass through untouched.
        _ => {}
    }
    msg
}

/// Push one projected harness event onto the session's outbound stream as a
/// JSON-RPC notification. `Error`/`TurnAborted` are adapted (they do not lift
/// directly via `event_msg_to_notification`); `Warning` is dropped.
fn push_event(notifier: &Notifier, thread_id: &str, msg: EventMsg) {
    match msg {
        EventMsg::Error(error) => notifier.notify(
            method::ERROR,
            &ErrorParams {
                thread_id: Some(thread_id.to_owned()),
                turn_id: None,
                item_id: None,
                code: error.code.unwrap_or_else(|| "error".to_owned()),
                message: error.message,
                data: error.data,
            },
        ),
        EventMsg::TurnAborted(aborted) => notifier.notify(
            method::TURN_COMPLETED,
            &TurnCompletedParams {
                thread_id: aborted.thread_id,
                turn: aborted.turn,
                usage: aborted.usage,
                reason: aborted.reason,
            },
        ),
        EventMsg::Warning(_) => {}
        other => {
            if let Some(notification) = event_msg_to_notification(other) {
                notifier.notify(notification.method(), &notification.payload());
            }
        }
    }
}

/// Decision returned by [`dedupe_fanout`].
enum FanoutDedupe {
    /// A live fan-out task already exists for this real thread id.
    Skip,
    /// No live task (absent, or the previous task finished) — spawn one.
    Spawn,
}

/// Guard the "at most one live fan-out task per real thread id" invariant. A
/// still-running task yields [`FanoutDedupe::Skip`]; a finished task (e.g. its
/// broadcast receiver hit `Closed`) is treated like an absent entry so a
/// re-resume after task death can re-establish the fan-out.
fn dedupe_fanout(tasks: &HashMap<String, JoinHandle<()>>, real_id: &str) -> FanoutDedupe {
    match tasks.get(real_id) {
        Some(handle) if !handle.is_finished() => FanoutDedupe::Skip,
        _ => FanoutDedupe::Spawn,
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::protocol::{ContextCompactedParams, ContextCompactingParams, ErrorEvent};
    use slab_jsonrpc::JSONRPCMessage;
    use tokio::sync::mpsc;

    use super::*;

    #[test]
    fn error_event_pushes_error_notification_with_thread_and_code() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let notifier = Notifier::new(tx);
        push_event(
            &notifier,
            "t1",
            EventMsg::Error(ErrorEvent::new("boom").with_code("turn_failed")),
        );
        let JSONRPCMessage::Notification(n) = rx.blocking_recv().expect("notification queued")
        else {
            panic!("expected notification");
        };
        assert_eq!(n.method, "error");
        let params = n.params.expect("params");
        assert_eq!(params["threadId"], "t1");
        assert_eq!(params["code"], "turn_failed");
    }

    #[test]
    fn rewrite_thread_id_rewrites_compaction_events() {
        let compacting = rewrite_thread_id(
            EventMsg::ContextCompacting(ContextCompactingParams { thread_id: "real-1".to_owned() }),
            "hthread-1",
        );
        match compacting {
            EventMsg::ContextCompacting(p) => assert_eq!(p.thread_id, "hthread-1"),
            other => panic!("unexpected variant: {other:?}"),
        }

        let compacted = rewrite_thread_id(
            EventMsg::ContextCompacted(ContextCompactedParams {
                thread_id: "real-1".to_owned(),
                status: Some("compacted".to_owned()),
                removed_messages: Some(2),
                output_tokens: Some(80),
            }),
            "hthread-1",
        );
        match compacted {
            EventMsg::ContextCompacted(p) => assert_eq!(p.thread_id, "hthread-1"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn dedupe_fanout_spawns_when_absent_and_skips_when_live() {
        use tokio::sync::mpsc;

        let tasks: HashMap<String, JoinHandle<()>> = HashMap::new();
        // Absent real id → Spawn (also exercises the finished/`_` arm).
        assert!(matches!(dedupe_fanout(&tasks, "real-1"), FanoutDedupe::Spawn));

        // A task parked on a channel stays live for the whole test.
        let (block_tx, mut block_rx) = mpsc::channel::<()>(1);
        let mut tasks = tasks;
        tasks.insert(
            "real-live".to_owned(),
            tokio::spawn(async move {
                let _ = block_rx.recv().await;
            }),
        );

        // Live real id → Skip; any other id still → Spawn.
        assert!(matches!(dedupe_fanout(&tasks, "real-live"), FanoutDedupe::Skip));
        assert!(matches!(dedupe_fanout(&tasks, "real-other"), FanoutDedupe::Spawn));

        // Unblock so the parked task can finish before the runtime shuts down.
        let _ = block_tx.send(()).await;
    }
}
