//! Outbound notification capability for a JSON-RPC 2.0 session.
//!
//! [`Notifier`] wraps the multi-producer channel that feeds a session's
//! outbound stream (see [`crate::ws::serve_websocket`]). Anything that needs to
//! push server→client notifications — a request handler, a long-lived fan-out
//! task, or a future server-initiated push (cron, background completion) —
//! holds a [`Notifier`] cloned from the session and calls [`Notifier::notify`].
//!
//! The underlying channel is an implementation detail; callers never touch it
//! directly. [`Notifier`] is a cheap `Clone` (an `Arc` inside). A
//! [`NotifierRegistry`] keys weak notifier handles by `session_id` so
//! server-initiated pushes can fan out to a session's active connections
//! without keeping a closed connection's channel alive.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, Weak};

use serde::Serialize;
use tokio::sync::mpsc;

use crate::{JSONRPCMessage, JSONRPCNotification};

/// Handle that pushes JSON-RPC notifications onto a session's outbound stream.
///
/// Clone it freely: one per fan-out task, one in the request context, one
/// handed to a [`NotifierRegistry`] for server-initiated pushes. All clones
/// share the same underlying channel.
#[derive(Clone, Debug)]
pub struct Notifier {
    inner: Arc<NotifierInner>,
}

#[derive(Debug)]
struct NotifierInner {
    tx: mpsc::UnboundedSender<JSONRPCMessage>,
}

impl Notifier {
    /// Wrap an outbound channel sender. The caller (session/socket setup)
    /// constructs this from the same channel whose receiver is handed to
    /// [`crate::ws::serve_websocket`].
    pub fn new(tx: mpsc::UnboundedSender<JSONRPCMessage>) -> Self {
        Self { inner: Arc::new(NotifierInner { tx }) }
    }

    /// Push a `method` / `params` notification onto the outbound stream.
    ///
    /// `params` is serialized here; on serialization failure the message is
    /// dropped with a `warn` log (a non-serializable payload is a programmer
    /// error, not a transport failure). A send failure means the session's
    /// outbound receiver is gone (socket closed) — silently ignored, since
    /// outstanding fan-out tasks are expected to stop once their event stream
    /// closes.
    pub fn notify<M, P>(&self, method: M, params: &P)
    where
        M: AsRef<str>,
        P: Serialize,
    {
        let payload = match serde_json::to_value(params) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(
                    method = %method.as_ref(),
                    %error,
                    "failed to serialize notification params; dropping",
                );
                return;
            }
        };
        let message = JSONRPCMessage::Notification(JSONRPCNotification {
            method: method.as_ref().to_owned(),
            params: Some(payload),
        });
        let _ = self.inner.tx.send(message);
    }

    /// Downgrade to a [`WeakNotifier`] — for a registry that should NOT keep
    /// the channel alive once the owning connection drops.
    pub fn downgrade(&self) -> WeakNotifier {
        WeakNotifier { inner: Arc::downgrade(&self.inner) }
    }
}

/// A weak reference to a [`Notifier`]. Used by [`NotifierRegistry`] so closed
/// connections are reaped automatically.
#[derive(Clone, Debug)]
pub struct WeakNotifier {
    inner: Weak<NotifierInner>,
}

impl WeakNotifier {
    /// Upgrade to a strong [`Notifier`] if the owning connection is still live.
    pub fn upgrade(&self) -> Option<Notifier> {
        self.inner.upgrade().map(|inner| Notifier { inner })
    }
}

/// `session_id` → weak notifiers for that session's active connections.
///
/// Server-initiated pushes (cron jobs, background completion) look up a
/// session's notifiers here and fan out to every active connection. Dead
/// handles (whose owning connection has dropped) are reaped on read, so no
/// explicit revoke is needed.
#[derive(Debug, Default)]
pub struct NotifierRegistry {
    sessions: StdMutex<HashMap<String, Vec<WeakNotifier>>>,
}

impl NotifierRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a connection's notifier under its `session_id`. Stores a weak
    /// handle; the registry never keeps a closed connection's channel alive.
    pub fn register(&self, session_id: &str, notifier: &Notifier) {
        let mut sessions = self.sessions.lock().expect("notifier registry poisoned");
        sessions.entry(session_id.to_owned()).or_default().push(notifier.downgrade());
    }

    /// Snapshot of live notifiers for `session_id` (one per active connection),
    /// reaping any whose owning connection has dropped. Empty if the session has
    /// no active connections.
    pub fn for_session(&self, session_id: &str) -> Vec<Notifier> {
        let mut sessions = self.sessions.lock().expect("notifier registry poisoned");
        let Some(weaks) = sessions.get_mut(session_id) else {
            return Vec::new();
        };
        let live: Vec<Notifier> = weaks.iter().filter_map(WeakNotifier::upgrade).collect();
        weaks.retain(|w| w.inner.strong_count() > 0);
        if weaks.is_empty() {
            sessions.remove(session_id);
        }
        live
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use tokio::sync::mpsc;

    use super::{Notifier, NotifierRegistry};
    use crate::JSONRPCMessage;

    #[tokio::test]
    async fn notify_pushes_a_notification_with_serialized_params() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let notifier = Notifier::new(tx);

        notifier.notify("turn/completed", &json!({ "threadId": "t1", "turn": { "id": "0" } }));

        let message = rx.recv().await.expect("notification queued");
        match message {
            JSONRPCMessage::Notification(notification) => {
                assert_eq!(notification.method, "turn/completed");
                assert_eq!(notification.params.unwrap()["threadId"], "t1");
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn clones_share_one_channel_and_preserve_order() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let a = Notifier::new(tx);
        let b = a.clone();

        a.notify("first", &Value::Null);
        b.notify("second", &Value::Null);

        let first = rx.recv().await.expect("first");
        let second = rx.recv().await.expect("second");
        let methods: Vec<String> = [first, second]
            .into_iter()
            .map(|message| match message {
                JSONRPCMessage::Notification(notification) => notification.method,
                other => panic!("expected notification, got {other:?}"),
            })
            .collect();
        assert_eq!(methods, vec!["first".to_owned(), "second".to_owned()]);
    }

    #[tokio::test]
    async fn send_failure_when_receiver_dropped_is_silent() {
        let (tx, rx) = mpsc::unbounded_channel();
        let notifier = Notifier::new(tx);
        drop(rx); // simulate socket closed

        // Must not panic; the message is silently dropped.
        notifier.notify("noop", &Value::Null);
    }

    #[test]
    fn registry_returns_live_notifiers_and_reaps_dead() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let notifier = Notifier::new(tx);
        let registry = NotifierRegistry::new();

        registry.register("s1", &notifier);
        assert_eq!(registry.for_session("s1").len(), 1);

        drop(notifier);
        // Dead handle reaped on read.
        assert_eq!(registry.for_session("s1").len(), 0);
    }

    #[test]
    fn registry_fans_out_to_multiple_connections_of_a_session() {
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();
        let n1 = Notifier::new(tx1);
        let n2 = Notifier::new(tx2);
        let registry = NotifierRegistry::new();

        registry.register("shared", &n1);
        registry.register("shared", &n2);
        assert_eq!(registry.for_session("shared").len(), 2);
        // Dropping one connection leaves the other live.
        drop(n1);
        assert_eq!(registry.for_session("shared").len(), 1);
    }
}
