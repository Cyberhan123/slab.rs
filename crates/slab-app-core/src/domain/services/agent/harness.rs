//! Harness service — engine host for `/v1/agent/harness` (WS control plane).
//!
//! Drives the slab-agent turn loop and exposes thread lifecycle / control
//! operations. Holds a cheap clone of the shared [`AgentCore`].

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use slab_agent::CompactContext;
use slab_agent::CompactOutcome;
use slab_agent::config::AgentConfig;
use slab_agent::port::{
    ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, TurnItemRecord, TurnStateRecord,
};
use slab_types::ConversationMessage;
use slab_utils::session_snapshot::{
    build_migration_snapshot, project_id_from_root, write_session_snapshot_atomic,
};
use uuid::Uuid;

use super::{AgentCore, RestoredAgentSession};
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::AgentEventMsgSubscription;

/// Engine-side agent service: owns the turn-loop control surface consumed by
/// the harness WebSocket transport.
#[derive(Clone)]
pub struct HarnessService(AgentCore);

/// Outcome of a workspace migration preparation (B-8).
#[derive(Debug, Clone)]
pub struct WorkspaceMigrationOutcome {
    pub project_id: String,
    pub suspended_count: usize,
}

impl HarnessService {
    pub(crate) fn new(core: AgentCore) -> Self {
        Self(core)
    }

    // ----- shared-surface delegations (used by harness handlers) -------------

    /// Spawn a root agent thread. Returns the new thread ID.
    pub async fn spawn(
        &self,
        session_id: String,
        config: AgentConfig,
        messages: Vec<ConversationMessage>,
    ) -> Result<String, AppCoreError> {
        self.0.spawn(session_id, config, messages).await
    }

    /// Append user input to an existing agent thread and run the next turn.
    pub async fn send_input(&self, thread_id: &str, content: String) -> Result<(), AppCoreError> {
        self.0.send_input(thread_id, content).await
    }

    /// Subscribe to the harness-protocol (`EventMsg`) stream for a thread.
    ///
    /// Carries slab-agent's harness protocol surface (turn lifecycle / text /
    /// reasoning / tool items).
    pub fn subscribe_event_msgs(&self, thread_id: &str) -> AgentEventMsgSubscription {
        self.0.subscribe_event_msgs(thread_id)
    }

    /// Shared compaction policy (the same `Arc` wired into the agent turn loop),
    /// exposed so the HTTP chat/responses paths can reuse it for auto-compaction.
    pub(crate) fn compact_port(&self) -> Arc<dyn slab_agent::CompactPort> {
        Arc::clone(self.0.compact())
    }

    /// Restore the latest root thread for a chat session and its persisted messages.
    pub async fn restore_session(
        &self,
        session_id: &str,
    ) -> Result<RestoredAgentSession, AppCoreError> {
        self.0.restore_session(session_id).await
    }

    /// List persisted messages for a thread in replay order.
    pub async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AppCoreError> {
        self.0.list_thread_messages(thread_id).await
    }

    // ----- harness-specific control surface ---------------------------------

    /// Gracefully shut down a running agent thread.
    pub async fn shutdown(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.0.runtime().shutdown(thread_id).await.map_err(AppCoreError::from)
    }

    /// Interrupt the currently running turn while keeping the thread resumable.
    pub async fn interrupt(&self, thread_id: &str) -> Result<(), AppCoreError> {
        self.0.runtime().interrupt(thread_id).await.map_err(AppCoreError::from)
    }

    /// Set the per-session permission mode for a thread (flows from the harness
    /// `thread/start` / `turn/start` `permission_mode` param).
    pub async fn set_thread_mode(&self, thread_id: &str, mode: slab_exec_policy::PermissionMode) {
        self.0.runtime().control().set_thread_mode(thread_id, mode).await;
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
        self.0.events().approve_call(thread_id, call_id, approved, scope)
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
        let control = self.0.runtime().control();
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
        self.0
            .store()
            .archive_thread(thread_id, Some(&now))
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.0.get_thread_snapshot(thread_id).await
    }

    /// Rollback a thread to the state *through* `to_turn_index` (inclusive):
    /// deletes persisted messages and turn states with `turn_index > to_turn_index`.
    /// Refuses while the thread is running — interrupt it first.
    pub async fn rollback_thread(
        &self,
        thread_id: &str,
        to_turn_index: u32,
    ) -> Result<ThreadSnapshot, AppCoreError> {
        let active = self.0.runtime().control().active_thread_ids().await;
        if active.iter().any(|id| id == thread_id) {
            return Err(AppCoreError::Internal(
                "thread is running; interrupt it before rolling back".to_owned(),
            ));
        }
        let from = to_turn_index
            .checked_add(1)
            .ok_or_else(|| AppCoreError::Internal("turn index overflow".to_owned()))?;
        self.0
            .store()
            .delete_turn_states_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.0
            .store()
            .delete_thread_messages_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.0
            .store()
            .delete_turn_items_from(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.0.get_thread_snapshot(thread_id).await
    }

    /// Compact a thread's persisted history via the shared compaction policy.
    ///
    /// Summarizes older turns into a single recap (falling back to a
    /// trailing-window trim if the summarization LLM call fails) and keeps the
    /// leading system prompt + a recent window verbatim. Persists the result by
    /// clearing the thread's messages / turn states / turn items and re-inserting
    /// the compacted set at sequential turn indexes — so the next `send_input`
    /// resumes from `turn_index = compacted.len()` (see `append_input`). Refuses
    /// while the thread is running; interrupt it first.
    ///
    /// Returns the refreshed snapshot, the number of removed messages, and the
    /// estimated token count of the compacted set.
    pub async fn compact_thread(
        &self,
        thread_id: &str,
        model_override: Option<String>,
    ) -> Result<(ThreadSnapshot, u32, u32), AppCoreError> {
        let active = self.0.runtime().control().active_thread_ids().await;
        if active.iter().any(|id| id == thread_id) {
            return Err(AppCoreError::Internal(
                "thread is running; interrupt it before compacting".to_owned(),
            ));
        }

        let snapshot = self.0.get_thread_snapshot(thread_id).await?;
        let config: AgentConfig = serde_json::from_str(&snapshot.config_json).map_err(|e| {
            AppCoreError::Internal(format!("failed to deserialize agent config: {e}"))
        })?;
        let model_id = model_override.unwrap_or_else(|| config.model.clone());

        let mut records = self.0.list_thread_messages(thread_id).await?;
        records.sort_by(|left, right| {
            left.turn_index
                .cmp(&right.turn_index)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        let messages: Vec<ConversationMessage> =
            records.iter().map(|record| record.message.clone()).collect();

        let ctx = CompactContext {
            model_id: &model_id,
            summary_instructions: None,
            force: true,
            progress: None,
        };
        let outcome =
            self.0.compact().compact(&messages, &ctx).await.map_err(AppCoreError::from)?;
        let CompactOutcome::Replaced { messages: compacted, output_tokens, replaced_messages } =
            outcome
        else {
            // Skipped: history was already minimal — nothing to persist.
            return Ok((snapshot, 0, 0));
        };

        let store = self.0.store();
        store
            .delete_thread_messages_from(thread_id, 0)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        store
            .delete_turn_states_from(thread_id, 0)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        store
            .delete_turn_items_from(thread_id, 0)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;

        let created_at = Utc::now().to_rfc3339();
        for (index, message) in compacted.iter().enumerate() {
            let record = ThreadMessageRecord {
                id: format!("msg_{}_{}", Uuid::new_v4().simple(), index),
                thread_id: thread_id.to_owned(),
                turn_index: index as u32,
                message: message.clone(),
                created_at: created_at.clone(),
            };
            store
                .insert_thread_message(&record)
                .await
                .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        }

        let snapshot = self.0.get_thread_snapshot(thread_id).await?;
        Ok((snapshot, replaced_messages as u32, output_tokens as u32))
    }

    /// Return a persisted thread snapshot by id (used by `thread/resume`).
    pub async fn thread_snapshot(
        &self,
        thread_id: &str,
    ) -> Result<Option<ThreadSnapshot>, AppCoreError> {
        self.0.runtime().control().thread_snapshot(thread_id).await.map_err(AppCoreError::from)
    }

    /// List root agent threads with limit/cursor pagination (harness
    /// `thread/list`).
    pub async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AppCoreError> {
        self.0
            .store()
            .list_session_threads_filtered(session_id, filter)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// List persisted turn-state records for a thread ordered by `turn_index`.
    pub async fn list_turn_states(
        &self,
        thread_id: &str,
    ) -> Result<Vec<TurnStateRecord>, AppCoreError> {
        self.0
            .store()
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
            .0
            .store()
            .get_thread(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?
            .is_none()
        {
            return Err(AppCoreError::NotFound(format!("agent thread not found: {thread_id}")));
        }
        self.0
            .store()
            .list_turn_items(thread_id)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))
    }

    /// Return the number of currently active threads.
    #[allow(dead_code)]
    pub async fn active_thread_count(&self) -> usize {
        self.0.runtime().active_thread_count().await
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
        let suspended = self.0.runtime().interrupt_all().await;
        let project_id = project_id_from_root(workspace_root);
        let snapshot = build_migration_snapshot(&project_id, &suspended);
        write_session_snapshot_atomic(snapshot_dir, &snapshot).map_err(AppCoreError::Internal)?;
        Ok(WorkspaceMigrationOutcome { project_id, suspended_count: suspended.len() })
    }
}
