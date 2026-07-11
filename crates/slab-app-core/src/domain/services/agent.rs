//! Application service wrapping [`slab_agent::AgentRuntime`].
//!
//! Provides a stable, clone-friendly handle that the API handlers can extract
//! from [`AppState`][crate::context::AppState] via Axum's `State` extractor.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use dashmap::DashSet;
use slab_agent::AgentRuntime;
use slab_agent::config::AgentConfig;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentStorePort, ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, TurnItemRecord,
    TurnStateRecord,
};
use slab_types::ConversationMessage;
use slab_utils::session_snapshot::{
    build_migration_snapshot, project_id_from_root, write_session_snapshot_atomic,
};

use crate::application::agent::turn_item_persistence;
use crate::domain::models::{AgentCommand, AgentSessionSnapshot};
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::{AgentEventHub, AgentEventSubscription};

/// Thin wrapper around [`AgentRuntime`] that exposes an application-layer API.
#[derive(Clone)]
pub struct AgentService {
    runtime: AgentRuntime,
    store: Arc<dyn AgentStorePort>,
    events: Arc<AgentEventHub>,
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

impl AgentService {
    pub fn new(
        runtime: AgentRuntime,
        store: Arc<dyn AgentStorePort>,
        events: Arc<AgentEventHub>,
    ) -> Self {
        Self { runtime, store, events, turn_item_observers: Arc::new(DashSet::new()) }
    }

    /// Spawn a root agent thread.  Returns the new thread ID.
    pub async fn spawn(
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

    /// Handle one transport-neutral agent command.
    ///
    /// HTTP, WebSocket, and other callers should enter the agent use case here
    /// after converting their wire DTOs into [`AgentCommand`].
    pub async fn handle_command(&self, command: AgentCommand) -> Result<String, AppCoreError> {
        match command {
            AgentCommand::CreateResponse { session_id, config, messages } => {
                self.spawn(session_id, *config, messages).await
            }
            AgentCommand::AppendInput { thread_id, content } => {
                self.send_input(&thread_id, content).await?;
                Ok(thread_id)
            }
        }
    }

    pub async fn restore_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<AgentSessionSnapshot, AppCoreError> {
        let restored = self.restore_session(session_id).await?;
        Ok(AgentSessionSnapshot {
            session_id: session_id.to_owned(),
            thread: restored.thread,
            messages: restored.messages,
            responses: restored.responses,
        })
    }

    /// Get the current status of an agent thread.
    ///
    /// First checks the in-memory registry (for live threads), then falls back
    /// to the persisted snapshot so callers polling after completion still get
    /// an accurate status rather than a 404.
    pub async fn get_status(
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

    /// Gracefully shut down a running agent thread.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.runtime.shutdown(thread_id).await.map_err(AppCoreError::from)
    }

    /// Interrupt the currently running turn while keeping the thread resumable.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.runtime.interrupt(thread_id).await.map_err(AppCoreError::from)
    }

    /// Append user input to an existing agent thread and run the next turn.
    pub async fn send_input(&self, thread_id: &str, content: String) -> Result<(), AppCoreError> {
        self.runtime.append_input(thread_id, content).await.map_err(AppCoreError::from)?;
        self.ensure_turn_item_persistence(thread_id);
        Ok(())
    }

    /// List persisted root agent threads for a chat session, newest first.
    pub async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.store
            .list_session_threads(session_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// List root agent threads with limit/cursor pagination (harness
    /// `thread/list`).
    pub async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.store
            .list_session_threads_filtered(session_id, filter)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Restore the latest root thread for a chat session and its persisted messages.
    pub async fn restore_session(
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

    /// Fork a thread: clone its persisted history (messages + turn states) into
    /// a new child thread at `depth + 1`, without running a turn. An optional
    /// `model_override` replaces the parent's model on the cloned config.
    /// Returns the forked child snapshot.
    pub async fn fork_thread(
        &self,
        parent_thread_id: &str,
        model_override: Option<String>,
    ) -> Result<ThreadSnapshot, AppCoreError> {
        let control = self.runtime.control();
        let child_id = control
            .fork_thread(parent_thread_id, model_override)
            .await
            .map_err(AppCoreError::from)?;
        control
            .thread_snapshot(&child_id)
            .await
            .map_err(AppCoreError::from)?
            .ok_or_else(|| AppCoreError::Internal(format!("forked thread missing: {child_id}")))
    }

    /// Soft-archive a thread by stamping `archived_at`. Archived threads are
    /// excluded from `thread/list` unless the caller opts in via `include_archived`.
    pub async fn archive_thread(&self, thread_id: &str) -> Result<ThreadSnapshot, AppCoreError> {
        let now = Utc::now().to_rfc3339();
        self.store
            .archive_thread(thread_id, Some(&now))
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.get_thread_snapshot(thread_id).await
    }

    /// Rollback a thread to the state *through* `to_turn_index` (inclusive):
    /// deletes persisted messages and turn states with `turn_index > to_turn_index`.
    /// Refuses while the thread is running — interrupt it first.
    pub async fn rollback_thread(
        &self,
        thread_id: &str,
        to_turn_index: u32,
    ) -> Result<ThreadSnapshot, AppCoreError> {
        let active = self.runtime.control().active_thread_ids().await;
        if active.iter().any(|id| id == thread_id) {
            return Err(AppCoreError::Internal(
                "thread is running; interrupt it before rolling back".to_owned(),
            ));
        }
        let from = to_turn_index
            .checked_add(1)
            .ok_or_else(|| AppCoreError::Internal("turn index overflow".to_owned()))?;
        self.store
            .delete_turn_states_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.store
            .delete_thread_messages_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.store
            .delete_turn_items_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.get_thread_snapshot(thread_id).await
    }

    /// Return a persisted thread snapshot by id (used by `thread/resume`).
    pub async fn thread_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<Option<ThreadSnapshot>, AppCoreError> {
        self.runtime.control().thread_snapshot(thread_id).await.map_err(AppCoreError::from)
    }

    /// Fetch a thread snapshot or fail with `NotFound`.
    async fn get_thread_snapshot(&self, thread_id: &str) -> Result<ThreadSnapshot, AppCoreError> {
        self.store
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .ok_or_else(|| AppCoreError::NotFound(format!("agent thread not found: {thread_id}")))
    }

    /// List persisted messages for a thread in replay order.
    pub async fn list_thread_messages(
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

    /// List persisted turn-state records for a thread ordered by `turn_index`.
    pub async fn list_turn_states(
        &self,
        thread_id: &str,
    ) -> Result<Vec<TurnStateRecord>, AppCoreError> {
        self.store
            .list_turn_states(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// List persisted full-fidelity `TurnItem` snapshots for a thread, ordered
    /// by `(turn_index, seq)` for deterministic replay. Used by `thread/resume`.
    pub async fn list_turn_items(
        &self,
        thread_id: &str,
    ) -> Result<Vec<TurnItemRecord>, AppCoreError> {
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
            .list_turn_items(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Subscribe to the turn-event stream for a thread.
    ///
    /// Returns a broadcast receiver that replays events emitted after the call.
    pub fn subscribe_events(&self, thread_id: &str) -> AgentEventSubscription {
        self.events.subscribe_events(thread_id)
    }

    /// Send an approval decision for a pending tool-call.
    ///
    /// Both `thread_id` (from the URL path) and `call_id` must match so that
    /// approvals cannot be delivered to a different thread's pending call.
    ///
    /// Returns `true` if a pending approval with the given key was found and
    /// the decision was delivered.
    pub fn approve_call(
        &self,
        thread_id: &str,
        call_id: &str,
        approved: bool,
        scope: slab_exec_policy::ApprovalScope,
    ) -> bool {
        self.events.approve_call(thread_id, call_id, approved, scope)
    }

    /// Set the per-session permission mode for a thread (flows from the harness
    /// `thread/start` / `turn/start` `permission_mode` param).
    pub async fn set_thread_mode(&self, thread_id: &str, mode: slab_exec_policy::PermissionMode) {
        self.runtime.control().set_thread_mode(thread_id, mode).await;
    }

    /// Return the number of currently active threads.
    #[allow(dead_code)]
    pub async fn active_thread_count(&self) -> usize {
        self.runtime.active_thread_count().await
    }

    /// Prepare a workspace switch (B-8 / INFRA-01): interrupt every active agent
    /// thread, then write a project-scoped atomic snapshot of the interrupted
    /// threads so a future restore only resumes threads that belong to the
    /// originating workspace. Returns the project id + the number of threads that
    /// were suspended. Any failure aborts the migration (the caller must not
    /// proceed to switch workspaces on error).
    pub async fn prepare_workspace_migration(
        &self,
        workspace_root: &Path,
        snapshot_dir: &Path,
    ) -> Result<WorkspaceMigrationOutcome, AppCoreError> {
        let suspended = self.runtime.interrupt_all().await;
        let project_id = project_id_from_root(workspace_root);
        let snapshot = build_migration_snapshot(&project_id, &suspended);
        write_session_snapshot_atomic(snapshot_dir, &snapshot).map_err(AppCoreError::Internal)?;
        Ok(WorkspaceMigrationOutcome { project_id, suspended_count: suspended.len() })
    }

    pub(crate) fn runtime(&self) -> AgentRuntime {
        self.runtime.clone()
    }
}

/// Outcome of a workspace migration preparation (B-8).
#[derive(Debug, Clone)]
pub struct WorkspaceMigrationOutcome {
    pub project_id: String,
    pub suspended_count: usize,
}
