//! Agent application services.
//!
//! Split along slab-agent's two distinct external interfaces, inspired by the
//! Codex Model/Engine layering (but shaped by slab-agent's own callers, not a
//! verbatim copy of Codex):
//! - [`HarnessService`] drives the turn loop for `/v1/agent/harness` (WS).
//! - [`ResponseService`] produces the OpenAI Responses wire for `/responses`.
//!
//! Both services hold a cheap clone of the shared [`AgentCore`]; they do not
//! wrap each other.

pub mod compact;
pub mod harness;
pub mod response;

pub use compact::{SummarizingCompactPort, maybe_compact_messages};
pub use harness::HarnessService;
pub use response::ResponseService;

use std::sync::Arc;

use dashmap::DashSet;
use slab_agent::AgentRuntime;
use slab_agent::CompactPort;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{AgentStorePort, ThreadMessageRecord, ThreadSnapshot};
use slab_types::ConversationMessage;

use crate::error::AppCoreError;
use crate::infra::agent::event_hub::{AgentEventHub, AgentEventMsgSubscription};
use crate::infra::agent::turn_item_persistence;

/// Shared core held by both the harness and response services.
///
/// INVARIANT: every field must be `Arc`-backed — cloning yields another handle
/// to the *same* state. The two services therefore share one runtime, one store,
/// one event hub, and one `turn_item_observers` guard set. Adding any
/// owned/non-`Arc` field would silently make them diverge (especially the
/// `turn_item_observers` idempotency relied on by [`Self::spawn`] /
/// [`Self::send_input`]).
#[derive(Clone)]
pub(crate) struct AgentCore {
    runtime: AgentRuntime,
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
    /// Shared compaction policy used by the harness turn loop (via
    /// `AgentControl`), the manual `thread/compact/start` op, and the HTTP
    /// chat/responses paths. Same `Arc` instance as the one wired into
    /// `AgentControl` so all paths compact identically.
    compact: Arc<dyn CompactPort>,
    /// Thread ids that already have a turn-item persistence observer running.
    /// Guards `spawn_turn_item_persistence` to one observer per thread.
    turn_item_observers: Arc<DashSet<String>>,
}

/// Persisted session state restored by the unified agent responses route.
pub struct RestoredAgentSession {
    pub thread: Option<ThreadSnapshot>,
    pub messages: Vec<ThreadMessageRecord>,
    /// Complete OpenAI-Responses-canonical `Response` JSON objects, one per
    /// agent run, oldest first. Empty for pre-migration history.
    pub responses: Vec<serde_json::Value>,
}

impl AgentCore {
    pub(crate) fn new(
        runtime: AgentRuntime,
        store: Arc<dyn AgentStorePort>,
        events: Arc<AgentEventHub>,
        compact: Arc<dyn CompactPort>,
    ) -> Self {
        Self { runtime, store, events, compact, turn_item_observers: Arc::new(DashSet::new()) }
    }

    pub(crate) fn runtime(&self) -> AgentRuntime {
        self.runtime.clone()
    }

    pub(crate) fn compact(&self) -> &Arc<dyn CompactPort> {
        &self.compact
    }

    pub(crate) fn store(&self) -> &Arc<dyn AgentStorePort> {
        &self.store
    }

    pub(crate) fn events(&self) -> &Arc<AgentEventHub> {
        &self.events
    }

    /// Spawn a root agent thread. Returns the new thread ID.
    pub(crate) async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AppCoreError> {
        let thread_id = self
            .runtime
            .create_response(session_id, config, messages)
            .await
            .map_err(AppCoreError::from)?;
        self.ensure_turn_item_persistence(&thread_id);
        Ok(thread_id)
    }

    /// Ensure exactly one turn-item persistence observer is running for the
    /// thread. The first call for a given thread spawns it; subsequent calls
    /// (e.g. `send_input` resuming a thread) are no-ops. The observer runs for
    /// the process lifetime, capturing every finalized `TurnItem` across all of
    /// the thread's runs.
    fn ensure_turn_item_persistence(&self, real_thread_id: &str) {
        if self.turn_item_observers.insert(real_thread_id.to_owned()) {
            turn_item_persistence::spawn_turn_item_persistence(
                Arc::clone(&self.store),
                Arc::clone(&self.events),
                real_thread_id.to_owned(),
            );
        }
    }

    /// Append user input to an existing agent thread and run the next turn.
    pub(crate) async fn send_input(
        &self,
        thread_id: &str,
        content: String,
    ) -> Result<(), AppCoreError> {
        self.runtime.append_input(thread_id, content).await.map_err(AppCoreError::from)?;
        self.ensure_turn_item_persistence(thread_id);
        Ok(())
    }

    /// Restore the latest root thread for a chat session and its persisted messages.
    pub(crate) async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<RestoredAgentSession, AppCoreError> {
        let thread = self.list_session_threads(session_id).await?.into_iter().next();
        let messages = match thread.as_ref() {
            Some(thread) => self.list_thread_messages(&thread.id).await?,
            None => Vec::new(),
        };
        // response_json persistence was removed — only complete messages and
        // turn state are stored now. The field is retained for interface
        // stability (`/v1/sessions/{id}/agent-history` still returns it, empty).
        let responses = Vec::new();
        Ok(RestoredAgentSession { thread, messages, responses })
    }

    /// List persisted root agent threads for a chat session, newest first.
    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.store
            .list_session_threads(session_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// List persisted messages for a thread in replay order.
    pub(crate) async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AppCoreError> {
        if self
            .store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .is_none()
        {
            return Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}")));
        }

        self.store
            .list_thread_messages(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Fetch a thread snapshot or fail with `NotFound`.
    pub(crate) async fn get_thread_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<ThreadSnapshot, AppCoreError> {
        self.store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .ok_or_else(|| AppCoreError::NotFound(format!("agent thread not found: {thread_id}")))
    }

    /// Get the current status of an agent thread.
    ///
    /// First checks the in-memory registry (for live threads), then falls back
    /// to the persisted snapshot so callers polling after completion still get
    /// an accurate status rather than a 404.
    #[allow(dead_code)]
    pub(crate) async fn get_status(
        &self,
        thread_id: &str,
    ) -> Result<slab_types::agent::AgentThreadStatus, AppCoreError> {
        // Try the live in-memory registry first.
        match self.runtime.subscribe(thread_id).await {
            Ok(rx) => {
                return Ok(*rx.borrow());
            }
            Err(AgentError::ThreadNotFound(_)) => {
                // Thread has already finished and was removed from the registry.
                // Fall through to the DB lookup below.
            }
            Err(e) => return Err(AppCoreError::from(e)),
        }

        // Fallback: look up the persisted snapshot.
        match self.store.get_thread(thread_id).await {
            Ok(Some(snapshot)) => Ok(snapshot.status),
            Ok(None) => Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}"))),
            Err(e) => Err(AppCoreError::Internal(e.to_string())),
        }
    }

    /// Subscribe to the harness-protocol (`EventMsg`) stream for a thread.
    ///
    /// Returns a replay+live subscription carrying slab-agent's harness protocol
    /// (turn lifecycle / text / reasoning / tool items). Consumed by the harness
    /// WS fan-out and turn-item persistence.
    pub(crate) fn subscribe_event_msgs(&self, thread_id: &str) -> AgentEventMsgSubscription {
        self.events.subscribe_event_msgs(thread_id)
    }
}
