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

    /// Surface the outbound notifier (for future server-initiated pushes via a
    /// `NotifierRegistry`; not used by request handlers today).
    #[allow(dead_code)]
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
    /// ⚠️ The caller must ensure at most one fan-out task per real thread id
    /// (a duplicate would double-deliver every event to the client).
    pub(crate) fn spawn_event_fanout(&self, real_thread_id: String, harness_thread_id: String) {
        let service = self.inner.service.clone();
        let notifier = self.inner.notifier.clone();
        tokio::spawn(async move {
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
    }
}

/// Rewrite the top-level `thread_id` of an [`EventMsg`] from the real slab
/// thread id to the harness-visible thread id. Nested `TurnItem`s carry no
/// `thread_id`, so only the params' `thread_id` field is rewritten; `turn_id`
/// and item ids are preserved.
fn rewrite_thread_id(mut msg: EventMsg, harness_id: &str) -> EventMsg {
    let tid = harness_id.to_owned();
    match &mut msg {
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
            &TurnCompletedParams { thread_id: aborted.thread_id, turn: aborted.turn },
        ),
        EventMsg::Warning(_) => {}
        other => {
            if let Some(notification) = event_msg_to_notification(other) {
                notifier.notify(notification.method(), &notification.payload());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::protocol::ErrorEvent;
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
}
