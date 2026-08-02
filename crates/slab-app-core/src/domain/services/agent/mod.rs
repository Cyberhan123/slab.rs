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
#[cfg(test)]
mod harness_tests;
pub mod response;

pub use compact::{SummarizingCompactPort, maybe_compact_messages};
pub use harness::HarnessService;
pub use response::ResponseService;

use std::path::{Path, PathBuf};
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
use crate::infra::agent::rollout_persistence;

/// Shared core held by both the harness and response services.
///
/// INVARIANT: every field must be `Arc`-backed — cloning yields another handle
/// to the *same* state. The two services therefore share one runtime, one store,
/// one event hub, one rollout true source, and one `rollout_observers` guard
/// set. Adding any owned/non-`Arc` field would silently make them diverge
/// (especially the `rollout_observers` idempotency relied on by [`Self::spawn`]
/// / [`Self::send_input`]).
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
    /// Append-only rollout event-source true source (Slice 4). Shared with the
    /// harness so `compact_thread` / `fork_thread` / `rollback_thread` can
    /// access the rollout directly (Slice 6).
    rollout: Arc<slab_agent_rollout::RolloutFileStore>,
    /// The trace directory configured from `agent.debug` (Slice 11b), threaded
    /// in so the harness can apply the SAME root-vs-child `trace_path` rule as
    /// `RolloutBackedAgentStore::upsert_thread` when it reconstructs a
    /// `SessionMeta` (J4: fork / compact fallback). `None` when agent debugging
    /// is off — then even a root thread carries no `trace_path`.
    trace_dir: Option<PathBuf>,
    /// Thread ids that already have a rollout persistence observer running.
    /// Guards `spawn_rollout_persistence` to one observer per thread.
    rollout_observers: Arc<DashSet<String>>,
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
        rollout: Arc<slab_agent_rollout::RolloutFileStore>,
        trace_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            runtime,
            store,
            events,
            compact,
            rollout,
            trace_dir,
            rollout_observers: Arc::new(DashSet::new()),
        }
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

    /// Rollout true source accessor. Consumed by the harness `compact_thread`
    /// (truncate + `Compacted` append) and `rollback_thread` (single atomic
    /// `truncate_from_turn`) paths so those operations act on the rollout file
    /// directly instead of going through the store adapter's per-table deletes.
    pub(crate) fn rollout(&self) -> &Arc<slab_agent_rollout::RolloutFileStore> {
        &self.rollout
    }

    /// The configured trace directory (Slice 11b), so the harness can apply the
    /// canonical root-vs-child `trace_path` rule when reconstructing a
    /// `SessionMeta` (J4). `None` when agent debugging is off.
    pub(crate) fn trace_dir(&self) -> Option<&Path> {
        self.trace_dir.as_deref()
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
        self.ensure_rollout_persistence(&thread_id);
        Ok(thread_id)
    }

    /// Ensure exactly one rollout persistence observer is running for the
    /// thread. The first call for a given thread spawns it; subsequent calls
    /// (e.g. `send_input` resuming a thread) are no-ops. The observer runs for
    /// the process lifetime, capturing every finalized `TurnItem`, compaction
    /// marker, and allowed lifecycle event across all of the thread's runs.
    fn ensure_rollout_persistence(&self, real_thread_id: &str) {
        if self.rollout_observers.insert(real_thread_id.to_owned()) {
            rollout_persistence::spawn_rollout_persistence(
                Arc::clone(&self.rollout),
                Arc::clone(&self.events),
                real_thread_id.to_owned(),
                slab_agent_rollout::EventPersistenceMode::Limited,
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
        self.ensure_rollout_persistence(thread_id);
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
