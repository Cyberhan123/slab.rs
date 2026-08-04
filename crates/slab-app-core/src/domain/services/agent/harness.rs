//! Harness service — engine host for `/v1/agent/harness` (WS control plane).
//!
//! Drives the slab-agent turn loop and exposes thread lifecycle / control
//! operations. Holds a cheap clone of the shared [`AgentCore`].

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use slab_agent::CompactContext;
use slab_agent::CompactOutcome;
use slab_agent::config::AgentConfig;
use slab_agent::port::{
    ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, TurnItemRecord, TurnStateRecord,
};
use slab_agent_rollout::{
    CompactedPayload, RolloutItem, RolloutLine, RolloutStore, TurnContextPayload,
    read_rollout_lines,
};
use slab_types::ConversationMessage;
use slab_utils::session_snapshot::{
    build_migration_snapshot, project_id_from_root, write_session_snapshot_atomic,
};

use super::{AgentCore, RestoredAgentSession};
use crate::error::AppCoreError;
use crate::infra::agent::event_hub::AgentEventMsgSubscription;
use crate::infra::agent::rollout_store::build_session_meta;

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
    ///
    /// `control.fork_thread` clones the parent history per-row through the store
    /// adapter (all messages, then all turn states, then all turn items). That
    /// batched order places every `TurnContext` line before every `TurnItem`
    /// line in the child rollout file, which breaks `read_turn_items`'s
    /// running-turn attribution heuristic (turn N's items inherit the highest
    /// turn seen so far). It also only enqueues `AddItems` into the child
    /// recorder — the child `SessionMeta` header is NOT durable until a flush.
    ///
    /// So after `control.fork_thread` we rebuild the child rollout file
    /// **unconditionally and wholesale** in the correct interleaved order, with
    /// the child `SessionMeta` (carrying `parent_id` provenance) reconstructed
    /// from the child thread snapshot (H1: never depend on the not-yet-durable
    /// child rollout file). The wholesale rewrite then flushes + atomically
    /// replaces the child file, so the child replays attribution-correct
    /// history and starts with a proper header (H2).
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

        let rollout = self.0.rollout();
        // Flush both parent and child so the parent's data lines are durable for
        // the wholesale read, and the child's batched adapter writes (and any
        // recorder-seeded SessionMeta) are flushed before we atomically replace
        // the child file.
        let _ = rollout.flush(parent_thread_id).await;
        let _ = rollout.flush(&child_id).await;

        // H1: build the child SessionMeta from the child snapshot (control
        // already upserted it), mirroring upsert_thread. The child recorder has
        // only buffered AddItems — its file may be absent or header-less — so
        // reading the child file for its header (the pre-fix path) lost the
        // header on EVERY fork.
        let child_snapshot =
            control.thread_snapshot(&child_id).await.map_err(AppCoreError::from)?.ok_or_else(
                || AppCoreError::Internal(format!("forked thread missing: {child_id}")),
            )?;
        let child_meta_line = RolloutLine::now(RolloutItem::SessionMeta(build_session_meta(
            &child_snapshot,
            self.0.trace_dir(),
        )));

        // H2: the wholesale rewrite is UNCONDITIONAL. For a rollout-native
        // parent (the common case) reuse its exact on-disk interleaved order —
        // the production write order read_turn_items / read_messages already
        // attribute correctly — swapping the parent SessionMeta for the child's.
        // For a legacy parent with no rollout file yet, rebuild from the adapter
        // reads (which fall back to SQL) in correct per-turn interleaved order.
        let parent_lines = read_rollout_lines(&rollout.resolve_path(parent_thread_id));
        let has_parent_rollout_data =
            parent_lines.iter().any(|l| !matches!(l.item, RolloutItem::SessionMeta(_)));
        let rebuilt = if has_parent_rollout_data {
            let mut v: Vec<RolloutLine> = Vec::with_capacity(parent_lines.len() + 1);
            v.push(child_meta_line);
            v.extend(
                parent_lines.into_iter().filter(|l| !matches!(l.item, RolloutItem::SessionMeta(_))),
            );
            v
        } else {
            rebuild_child_lines_from_legacy_parent(&self.0, parent_thread_id, child_meta_line)
                .await?
        };
        rollout
            .rewrite_session(&child_id, rebuilt)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;

        Ok(child_snapshot)
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
    /// drops every rollout line belonging to a turn `> to_turn_index`.
    ///
    /// Slice 6 collapses the old three-way `store.delete_*_from` (which each
    /// routed to a separate `truncate_from_turn`) into a single atomic rollout
    /// truncation. `keep_line` preserves the `SessionMeta` header unconditionally
    /// and gates every other line by its turn affiliation, so one call drops the
    /// messages, turn states, and turn items of the rolled-back turns together.
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
        // H4: rollback writes the rollout file directly, but reads flow through
        // the gate-aware adapter. An un-backfilled legacy thread (no rollout
        // file) would be a SILENT no-op here (truncate returns Ok early on a
        // missing file) yet report success — refuse cleanly instead so the
        // caller retries after startup backfill flips the thread rollout-native.
        if !self.0.rollout().file_exists(thread_id).await {
            return Err(AppCoreError::Internal(format!(
                "thread {thread_id} is not yet migrated to the rollout true source; \
                 retry after backfill completes"
            )));
        }
        let from = to_turn_index
            .checked_add(1)
            .ok_or_else(|| AppCoreError::Internal("turn index overflow".to_owned()))?;
        self.0
            .rollout()
            .truncate_from_turn(thread_id, from)
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;
        self.0.get_thread_snapshot(thread_id).await
    }

    /// Compact a thread's persisted history via the shared compaction policy.
    ///
    /// Summarizes older turns into a single recap (falling back to a
    /// trailing-window trim if the summarization LLM call fails) and keeps the
    /// leading system prompt + a recent window verbatim. Persists the result as a
    /// single atomic full-rewrite (H3) so the rollout file goes straight from
    /// its prior contents to exactly `[SessionMeta, Compacted]` — there is NO
    /// intermediate truncated state on disk. The `Compacted` line (carrying the
    /// compacted set, `status = "manual"`, `turn_index = 0`) becomes the new
    /// `read_messages` baseline — the replay rules clear the prior `TurnContext`
    /// and adopt `compacted_messages` when the status is not `"skipped"`. The
    /// compacted messages are NOT re-inserted as separate `MessageAppend` lines
    /// (the `Compacted` line carries them; re-inserting would duplicate them on
    /// the next read). Refuses while the thread is running (interrupt it first)
    /// and refuses an un-backfilled legacy thread (H4 — the rollout file is the
    /// only place the compact is visible to the gate-aware reads).
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
        // H4: compact writes the rollout file directly, but reads flow through
        // the gate-aware adapter. For an un-backfilled legacy thread the compact
        // would be invisible (reads fall back to SQL with un-compacted history),
        // so refuse cleanly until backfill completes.
        if !self.0.rollout().file_exists(thread_id).await {
            return Err(AppCoreError::Internal(format!(
                "thread {thread_id} is not yet migrated to the rollout true source; \
                 retry after backfill completes"
            )));
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

        // H3: persist the compaction as a SINGLE atomic full-rewrite so there is
        // no window where the file holds only [SessionMeta] (truncated, no
        // Compacted baseline). The pre-fix `truncate_from_turn(0)` (durable)
        // then `append(Compacted)` (only enqueued — NOT durable until a later
        // flush) meant a hard crash between compact_thread returning and the next
        // read dropped the ENTIRE conversation. rewrite_session flushes pending
        // writes, drops the writer handle, and atomically replaces the file with
        // exactly [SessionMeta, Compacted] — durable at return time.
        let rollout = self.0.rollout();
        let compacted_line = RolloutLine::now(RolloutItem::Compacted(CompactedPayload {
            thread_id: thread_id.to_owned(),
            compacted_messages: compacted.clone(),
            removed_messages: replaced_messages as u32,
            output_tokens: output_tokens as u32,
            status: "manual".to_owned(),
            turn_index: 0,
        }));
        // Flush first so the existing SessionMeta header (and any pending writes)
        // is durable, then recover the header line (preserving its timestamp).
        // H4 guaranteed the file exists, so a header is present; the snapshot
        // fallback guards the impossible-but-defensive empty case.
        let _ = rollout.flush(thread_id).await;
        let session_meta_line = read_rollout_lines(&rollout.resolve_path(thread_id))
            .into_iter()
            .find(|l| matches!(l.item, RolloutItem::SessionMeta(_)))
            .unwrap_or_else(|| {
                RolloutLine::now(RolloutItem::SessionMeta(build_session_meta(
                    &snapshot,
                    self.0.trace_dir(),
                )))
            });
        rollout
            .rewrite_session(thread_id, vec![session_meta_line, compacted_line])
            .await
            .map_err(|e| AppCoreError::Internal(e.to_string()))?;

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

// ── fork/compact helpers ───────────────────────────────────────────────────

/// Rebuild a child rollout line set from a **legacy** parent (one with no
/// rollout file yet, read through the gate-aware adapter which falls back to
/// SQL). Returns `[child_meta_line, ...data lines]` in the production
/// per-turn interleaved order (messages, then turn state, then items) so
/// `read_turn_items`'s running-turn heuristic and `read_messages`'s
/// replace/append replay both attribute the child's history correctly (H2).
async fn rebuild_child_lines_from_legacy_parent(
    core: &AgentCore,
    parent_thread_id: &str,
    child_meta_line: RolloutLine,
) -> Result<Vec<RolloutLine>, AppCoreError> {
    let messages = core
        .store()
        .list_thread_messages(parent_thread_id)
        .await
        .map_err(|e| AppCoreError::Internal(e.to_string()))?;
    let states = core
        .store()
        .list_turn_states(parent_thread_id)
        .await
        .map_err(|e| AppCoreError::Internal(e.to_string()))?;
    let items = core
        .store()
        .list_turn_items(parent_thread_id)
        .await
        .map_err(|e| AppCoreError::Internal(e.to_string()))?;

    // Union of turn indices across the three record sets, ordered ascending so
    // the rebuilt file walks turns in order (each turn's TurnContext stamps the
    // running turn before that turn's TurnItems).
    let mut turns: BTreeSet<u32> = BTreeSet::new();
    for m in &messages {
        turns.insert(m.turn_index);
    }
    for s in &states {
        turns.insert(s.turn_index);
    }
    for i in &items {
        turns.insert(i.turn_index);
    }

    let mut rebuilt: Vec<RolloutLine> =
        Vec::with_capacity(messages.len() + states.len() + items.len() + 1);
    rebuilt.push(child_meta_line);
    for turn in turns {
        for m in messages.iter().filter(|m| m.turn_index == turn) {
            rebuilt.push(RolloutLine::now(RolloutItem::TurnContext(
                TurnContextPayload::MessageAppend {
                    turn_index: turn,
                    message: m.message.clone(),
                    id: Some(m.id.clone()),
                    created_at: Some(m.created_at.clone()),
                },
            )));
        }
        for s in states.iter().filter(|s| s.turn_index == turn) {
            rebuilt.push(RolloutLine::now(RolloutItem::TurnContext(
                turn_state_payload_from_record(s),
            )));
        }
        for it in items.iter().filter(|i| i.turn_index == turn) {
            let item: slab_agent::protocol::TurnItem = serde_json::from_str(&it.item_json)
                .map_err(|e| AppCoreError::Internal(format!("failed to decode TurnItem: {e}")))?;
            rebuilt.push(RolloutLine::now(RolloutItem::TurnItem(item)));
        }
    }
    Ok(rebuilt)
}

/// Reconstruct a rollout [`TurnContextPayload::TurnState`] from a persisted
/// [`TurnStateRecord`], mirroring `RolloutBackedAgentStore::upsert_turn_state`
/// (including the F6 raw-blob preservation when the typed list fails to parse).
fn turn_state_payload_from_record(record: &TurnStateRecord) -> TurnContextPayload {
    let mut input_messages = Vec::new();
    let mut input_messages_raw = None;
    if let Some(json) = &record.input_messages_json {
        match serde_json::from_str::<Vec<ConversationMessage>>(json) {
            Ok(parsed) => input_messages = parsed,
            Err(_) => input_messages_raw = Some(json.clone()),
        }
    }
    TurnContextPayload::TurnState {
        turn_index: record.turn_index,
        status: record.status.clone(),
        input_messages,
        tool_specs_json: record.tool_specs_json.clone(),
        llm_response_json: record.llm_response_json.clone(),
        error: record.error.clone(),
        completed_at: record.completed_at.clone(),
        started_at: Some(record.started_at.clone()),
        input_messages_raw,
    }
}
