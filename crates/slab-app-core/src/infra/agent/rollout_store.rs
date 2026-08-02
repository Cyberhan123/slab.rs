//! Rollout-backed [`AgentStorePort`] adapter — the new true source for agent
//! conversation, turn state, and finalized items.
//!
//! Slice 4 wired this adapter as the **only** `AgentStorePort` impl in the
//! agent runtime; Slice 5 replaces the bare `rollout.file_exists` read gate
//! with a `rollout_session_index.backfill_status == "completed"` gate so a
//! legacy thread keeps reading from SQL until its backfill copies the legacy
//! rows into the rollout file (resolving the pre-W2B legacy-orphan window).
//!
//! # Routing
//! - **Thread metadata** (`upsert_thread` / `get_thread` / `list_session_threads` /
//!   `update_thread_status` / `archive_thread`) → always `SqlxStore`
//!   (`agent_threads` stays the metadata truth source). `upsert_thread` also
//!   stamps the rollout `SessionMeta` header on first upsert, and marks the
//!   thread rollout-native (`backfill_status = "completed"`) when it carries
//!   no legacy SQL conversation data — i.e. a brand-new thread born on rollout.
//! - **Tool-call audit** (`insert_tool_call` / `update_tool_call*`) → always
//!   `SqlxStore` (`agent_tool_calls` is still written).
//! - **Writes** (`insert_thread_message` / `upsert_turn_state` /
//!   `insert_turn_item`) → rollout (`TurnContext` / `TurnItem`). The legacy
//!   three tables are no longer written.
//! - **Reads** (`list_thread_messages` / `list_turn_items` / `list_turn_states`)
//!   → rollout-first ONLY when the index row is `backfill_status = "completed"`
//!   (a new thread stamped at creation, or a legacy thread whose startup
//!   backfill finished); otherwise fall back to `SqlxStore` so an
//!   un-backfilled legacy thread stays fully recoverable. There is no orphan
//!   window: a legacy thread reads SQL right up until its backfill flips the
//!   gate, then reads rollout.
//! - **Deletes** (`delete_*_from`) → `rollout.truncate_from_turn` (all three
//!   collapse to one atomic file truncation).
//!
//! # Known transient backfill-window gap (G5)
//! For a legacy thread that is actively used BEFORE its startup backfill
//! completes: writes go to the rollout file, but reads fall back to SQL (the
//! gate is not yet `completed`), so the just-written post-migration turn is
//! INVISIBLE to reads until backfill flips the gate. This is a TRANSIENT gap —
//! backfill runs once at startup and the legacy history is small on desktop, so
//! the window closes quickly. Once backfill completes, the gate flips and the
//! rollout read serves the merged legacy prefix + post-migration tail (so the
//! previously-invisible turn reappears). This is NOT the permanent orphan the
//! pre-G1 mixed-case shortcut caused (that shortcut dropped the legacy prefix
//! entirely once any post-migration write materialized the rollout file); G1's
//! atomic rewrite merges both, eliminating the permanent orphan.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentStorePort, ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, ToolCallRecord,
    TurnItemRecord, TurnStateRecord,
};
use slab_agent_rollout::{
    RolloutFileStore, RolloutItem, RolloutLine, RolloutStore, SessionMeta, TurnContextPayload,
    read_rollout_lines,
};
use slab_types::ConversationMessage;
use slab_types::agent::ToolCallStatus;

use crate::infra::db::repository::rollout_index::RolloutIndex;

/// `AgentStorePort` impl that backs reads/writes with the rollout JSONL true
/// source, delegating only metadata + tool-call audit to the SQL store.
pub struct RolloutBackedAgentStore {
    /// SQL delegate for thread metadata + tool-call audit (and the read
    /// fallback for threads not yet migrated to rollout).
    sqlx: Arc<dyn AgentStorePort>,
    /// app-core-internal handle over the `rollout_session_index` /
    /// `rollout_backfill_state` tables — backs the read gate and the
    /// new-thread mark. NOT on `AgentStorePort` (slab-agent stays pure).
    index: Arc<dyn RolloutIndex>,
    /// The append-only rollout true source.
    rollout: Arc<RolloutFileStore>,
    /// Slice 11b rollout ↔ trace coordination: the directory where the agent
    /// trace artifacts (legacy per-session JSONL today; the W4 trace bundle
    /// once wired into bootstrap) live, stamped onto the ROOT thread's
    /// [`SessionMeta::trace_path`] so a diagnostic can jump from the rollout
    /// file to the semantic trace. `None` when agent debugging is off.
    /// Child threads get `trace_path: None` and correlate back to their root
    /// thread's bundle via `root_thread_id`.
    trace_dir: Option<PathBuf>,
}

impl RolloutBackedAgentStore {
    /// Wrap a SQL `AgentStorePort` delegate with a rollout true source, a
    /// [`RolloutIndex`] handle (both satisfied by `SqlxStore` in production),
    /// and an optional trace directory for [`SessionMeta::trace_path`]
    /// coordination (Slice 11b). Pass `None` when agent debugging is off.
    pub fn new(
        sqlx: Arc<dyn AgentStorePort>,
        index: Arc<dyn RolloutIndex>,
        rollout: Arc<RolloutFileStore>,
        trace_dir: Option<PathBuf>,
    ) -> Self {
        Self { sqlx, index, rollout, trace_dir }
    }

    /// The Slice-5 read gate: a thread reads rollout-first ONLY once its
    /// `rollout_session_index.backfill_status` is `"completed"`.
    ///
    /// On an index lookup error, the gate resolves by FILE EXISTENCE rather
    /// than blindly falling back to SQL (G2): a rollout-native thread (born on
    /// rollout, no legacy SQL rows) has its data in the rollout file and an
    /// EMPTY SQL history — a blind SQL fallback would serve empty history. So
    /// when the index lookup errors, if the rollout file exists we treat the
    /// thread as rollout-ready; otherwise we fall back to SQL (a legacy thread
    /// whose data is in SQL).
    async fn is_rollout_ready(&self, thread_id: &str) -> bool {
        match self.index.rollout_backfill_status(thread_id).await {
            Ok(Some(status)) => status == "completed",
            Ok(None) => false,
            Err(error) => {
                // Resolve by file existence: a rollout-native thread has its
                // data in the rollout file (SQL empty); a legacy thread has its
                // data in SQL (no rollout file yet). This avoids serving empty
                // history for a new thread on a transient SQLite error.
                let file_exists = self.rollout.file_exists(thread_id).await;
                tracing::warn!(
                    thread_id = %thread_id,
                    %error,
                    file_exists,
                    "rollout_session_index lookup failed; resolving read gate by file existence",
                );
                file_exists
            }
        }
    }
}

/// Build a rollout [`SessionMeta`] header from a [`ThreadSnapshot`], applying
/// the SINGLE canonical root-vs-child `trace_path` rule: a ROOT thread (no
/// `parent_id`) stamped with the trace dir when agent debugging is on; a CHILD
/// thread always carries `None` (it correlates back to its root thread's trace
/// bundle via `root_thread_id`).
///
/// This is the ONE place the rule lives — both `RolloutBackedAgentStore::upsert_thread`
/// (first-upsert header stamp) and the harness `fork_thread` / `compact_thread`
/// reconstruction paths call it, so a root-thread compact fallback can no longer
/// silently drop `trace_path` and the two constructions cannot drift.
pub(crate) fn build_session_meta(
    snapshot: &ThreadSnapshot,
    trace_dir: Option<&Path>,
) -> SessionMeta {
    let config_json =
        serde_json::from_str(&snapshot.config_json).unwrap_or_else(|_| serde_json::json!({}));
    // Slice 11b: stamp trace_path ONLY on a root thread (no parent). Child
    // threads correlate back to their root thread's trace bundle via
    // root_thread_id, so they carry None and inherit the pointer.
    let trace_path = if snapshot.parent_id.is_none() {
        trace_dir.map(|dir| dir.to_string_lossy().into_owned())
    } else {
        None
    };
    SessionMeta {
        thread_id: snapshot.id.clone(),
        session_id: snapshot.session_id.clone(),
        parent_id: snapshot.parent_id.clone(),
        started_at: snapshot.created_at.clone(),
        config_json,
        rollout_version: SessionMeta::CURRENT_VERSION,
        role_name: snapshot.role_name.clone(),
        trace_path,
    }
}

#[async_trait]
impl AgentStorePort for RolloutBackedAgentStore {
    // ── Thread metadata (always SQL) ───────────────────────────────────────

    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
        // Metadata always goes to SQLite (agent_threads is the metadata truth).
        self.sqlx.upsert_thread(snapshot).await?;
        // Stamp the rollout session header on the first upsert for a thread —
        // when no rollout file exists yet. create_session is idempotent: a
        // second call (file already present) no-ops via Resume (M2).
        if !self.rollout.file_exists(&snapshot.id).await {
            self.rollout.create_session(build_session_meta(snapshot, self.trace_dir.as_deref()));
        }
        // Mark the thread rollout-native when it carries NO legacy SQL
        // conversation data — i.e. a brand-new thread born on rollout. A
        // legacy thread (pre-migration rows still in the three tables) is left
        // for the startup backfill to mark completed; marking it here would
        // flip the read gate to an empty rollout file and orphan the legacy
        // prefix. `upsert_thread` is creation-only in practice (thread spawn /
        // fork / single-shot), so this probe returns false for genuine new
        // threads and guards the rare re-upsert of a legacy id.
        let has_legacy = match self.index.thread_has_legacy_data(&snapshot.id).await {
            Ok(has_legacy) => has_legacy,
            Err(error) => {
                tracing::warn!(
                    thread_id = %snapshot.id,
                    %error,
                    "failed to probe legacy conversation data; skipping rollout_session_index mark",
                );
                return Ok(());
            }
        };
        if !has_legacy {
            let file_path = self.rollout.path_for(&snapshot.id).to_string_lossy().into_owned();
            if let Err(error) = self
                .index
                .mark_rollout_session(
                    &snapshot.id,
                    &snapshot.session_id,
                    &file_path,
                    0,
                    None,
                    0,
                    "completed",
                )
                .await
            {
                tracing::warn!(
                    thread_id = %snapshot.id,
                    %error,
                    "failed to stamp rollout_session_index; read gate will fall back to SQL",
                );
            }
        }
        Ok(())
    }

    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
        self.sqlx.get_thread(id).await
    }

    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        self.sqlx.list_session_threads(session_id).await
    }

    async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        self.sqlx.list_session_threads_filtered(session_id, filter).await
    }

    async fn update_thread_status(
        &self,
        id: &str,
        status: slab_agent::port::ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), AgentError> {
        self.sqlx.update_thread_status(id, status, completion_text).await
    }

    async fn archive_thread(&self, id: &str, archived_at: Option<&str>) -> Result<(), AgentError> {
        self.sqlx.archive_thread(id, archived_at).await
    }

    // ── Tool-call audit (always SQL) ───────────────────────────────────────

    async fn insert_tool_call(&self, record: &ToolCallRecord) -> Result<(), AgentError> {
        self.sqlx.insert_tool_call(record).await
    }

    async fn update_tool_call_status(
        &self,
        id: &str,
        status: ToolCallStatus,
    ) -> Result<(), AgentError> {
        self.sqlx.update_tool_call_status(id, status).await
    }

    async fn update_tool_call(
        &self,
        id: &str,
        output: Option<&str>,
        status: ToolCallStatus,
        completed_at: &str,
    ) -> Result<(), AgentError> {
        self.sqlx.update_tool_call(id, output, status, completed_at).await
    }

    // ── Writes → rollout (legacy three tables stop being written) ──────────

    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError> {
        self.rollout
            .append(
                &record.thread_id,
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: record.turn_index,
                    message: record.message.clone(),
                    // F3: carry the record id + created_at through the rollout so
                    // replay recovers the original values (frontends use message.id
                    // as a React key; the SQL fallback returns real ids, so without
                    // these the two paths diverge).
                    id: Some(record.id.clone()),
                    created_at: Some(record.created_at.clone()),
                }),
            )
            .await
            .map_err(|e| AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn upsert_turn_state(&self, record: &TurnStateRecord) -> Result<(), AgentError> {
        // Deserialize the persisted input-message blob back to the typed list
        // the rollout TurnState carries. Missing/empty → empty vec (the summary
        // baseline arrives via a separate MessageAppend or a later TurnState).
        // On parse failure (F6) the raw blob is preserved verbatim in
        // `input_messages_raw` so a malformed blob is recoverable instead of
        // being silently emptied (the SQL store preserved it verbatim).
        let mut input_messages = Vec::new();
        let mut input_messages_raw = None;
        if let Some(json) = &record.input_messages_json {
            match serde_json::from_str::<Vec<ConversationMessage>>(json) {
                Ok(parsed) => input_messages = parsed,
                Err(error) => {
                    tracing::warn!(
                        thread_id = %record.thread_id,
                        turn_index = record.turn_index,
                        error = %error,
                        "failed to parse input_messages_json; preserving raw blob",
                    );
                    input_messages_raw = Some(json.clone());
                }
            }
        }
        self.rollout
            .append(
                &record.thread_id,
                RolloutItem::TurnContext(TurnContextPayload::TurnState {
                    turn_index: record.turn_index,
                    status: record.status.clone(),
                    input_messages,
                    tool_specs_json: record.tool_specs_json.clone(),
                    llm_response_json: record.llm_response_json.clone(),
                    error: record.error.clone(),
                    completed_at: record.completed_at.clone(),
                    // F4: carry the real turn-start timestamp so replay recovers
                    // it (otherwise replay substitutes the line write-time).
                    started_at: Some(record.started_at.clone()),
                    // F6: the unparsed blob, when the typed list is empty.
                    input_messages_raw,
                }),
            )
            .await
            .map_err(|e| AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn insert_turn_item(&self, record: &TurnItemRecord) -> Result<(), AgentError> {
        let item: slab_agent::protocol::TurnItem = serde_json::from_str(&record.item_json)
            .map_err(|e| {
                AgentError::Store(format!("failed to decode TurnItem from record: {e}"))
            })?;
        self.rollout
            .append(&record.thread_id, RolloutItem::TurnItem(item))
            .await
            .map_err(|e| AgentError::Store(e.to_string()))?;
        Ok(())
    }

    // ── Reads → rollout-first (gate: backfill_status), SQL fallback ─────────
    //
    // The flush below runs UNCONDITIONALLY before the gate check (F2): the
    // recorder is lazy (it only writes on Persist/Shutdown/Truncate), so
    // without this flush a freshly-written thread's pending items are not
    // durable and the rollout read would miss them. The flush is a no-op when
    // no recorder exists.
    //
    // Slice-5 gate: a thread reads rollout-first ONLY when
    // `rollout_session_index.backfill_status == "completed"` (a new thread
    // stamped at creation, or a legacy thread whose startup backfill finished).
    // Otherwise the read delegates to SQL — so an un-backfilled legacy thread
    // stays fully recoverable and there is no orphan window (the legacy thread
    // reads SQL right up until its backfill flips the gate, then reads
    // rollout). This replaces the bare `rollout.file_exists` check, which
    // orphaned the legacy prefix the moment a post-migration write
    // materialized the rollout file.

    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        if self.is_rollout_ready(thread_id).await {
            let lines = read_rollout_lines(&self.rollout.path_for(thread_id));
            Ok(replay_messages(thread_id, &lines))
        } else {
            self.sqlx.list_thread_messages(thread_id).await
        }
    }

    async fn list_turn_items(&self, thread_id: &str) -> Result<Vec<TurnItemRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        if self.is_rollout_ready(thread_id).await {
            Ok(self.rollout.read_turn_items(thread_id).await)
        } else {
            self.sqlx.list_turn_items(thread_id).await
        }
    }

    async fn list_turn_states(&self, thread_id: &str) -> Result<Vec<TurnStateRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        if self.is_rollout_ready(thread_id).await {
            let lines = read_rollout_lines(&self.rollout.path_for(thread_id));
            Ok(replay_turn_states(thread_id, &lines))
        } else {
            self.sqlx.list_turn_states(thread_id).await
        }
    }

    // ── Deletes → single atomic rollout truncation ─────────────────────────
    //
    // All three collapse to one `truncate_from_turn` (drops every line at or
    // beyond `from_turn_index`). Idempotent: a second call is a no-op since the
    // targeted lines are already gone.

    async fn delete_turn_states_from(
        &self,
        thread_id: &str,
        from_turn_index: u32,
    ) -> Result<(), AgentError> {
        self.rollout
            .truncate_from_turn(thread_id, from_turn_index)
            .await
            .map_err(|e| AgentError::Store(e.to_string()))
    }

    async fn delete_thread_messages_from(
        &self,
        thread_id: &str,
        from_turn_index: u32,
    ) -> Result<(), AgentError> {
        self.rollout
            .truncate_from_turn(thread_id, from_turn_index)
            .await
            .map_err(|e| AgentError::Store(e.to_string()))
    }

    async fn delete_turn_items_from(
        &self,
        thread_id: &str,
        from_turn_index: u32,
    ) -> Result<(), AgentError> {
        self.rollout
            .truncate_from_turn(thread_id, from_turn_index)
            .await
            .map_err(|e| AgentError::Store(e.to_string()))
    }
}

/// Replay the LLM-visible conversation from rollout lines, materializing
/// [`ThreadMessageRecord`]s with the turn affiliation each message belonged to.
///
/// Mirrors the `RolloutStore::read_messages` semantics: a `Compacted` marker
/// resets the baseline (and, when non-empty, supplies the new one) — EXCEPT a
/// skipped compaction (`status == "skipped"`, attempted but no shrink) which is
/// a no-op and leaves the replayed baseline intact; a
/// `TurnState` with non-empty `input_messages` replaces the prior history;
/// `MessageAppend` increments it. Turn index and a per-thread sequence number
/// are attached so callers that rely on `(turn_index, ordering)` still work.
///
/// When a `MessageAppend` line carries the original record `id` / `created_at`
/// (F3), those are recovered verbatim; otherwise a stable synthetic id
/// (`{thread_id}-r{seq}`) and the line timestamp are used (backward-compat with
/// rollout files written before the F3 fields existed).
fn replay_messages(thread_id: &str, lines: &[RolloutLine]) -> Vec<ThreadMessageRecord> {
    // (message, turn_index, created_at, carried_id)
    let mut baseline: Vec<(ConversationMessage, u32, String, Option<String>)> = Vec::new();
    for line in lines {
        match &line.item {
            RolloutItem::Compacted(payload) => {
                // A skipped compaction (attempted but did not shrink) changes
                // nothing — keep the replayed baseline intact. Mirrors the
                // `status == "skipped"` no-op in `RolloutStore::read_messages`;
                // without this guard a skipped marker orphans the conversation on
                // the production read path (this `replay_messages`, NOT the
                // rollout crate's `read_messages` which no production caller uses).
                if payload.status == "skipped" {
                    continue;
                }
                baseline.clear();
                let turn_index = payload.turn_index;
                for msg in &payload.compacted_messages {
                    baseline.push((msg.clone(), turn_index, line.timestamp.clone(), None));
                }
            }
            RolloutItem::TurnContext(TurnContextPayload::TurnState {
                turn_index,
                input_messages,
                ..
            }) => {
                if !input_messages.is_empty() {
                    baseline = input_messages
                        .iter()
                        .map(|m| (m.clone(), *turn_index, line.timestamp.clone(), None))
                        .collect();
                }
            }
            RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                turn_index,
                message,
                id,
                created_at,
            }) => {
                let stamped = created_at.clone().unwrap_or_else(|| line.timestamp.clone());
                baseline.push((message.clone(), *turn_index, stamped, id.clone()));
            }
            // SessionMeta / TurnItem / EventMsg do not contribute to the
            // LLM-visible message list.
            _ => {}
        }
    }

    let mut seq = 0u32;
    baseline
        .into_iter()
        .map(|(message, turn_index, created_at, carried_id)| {
            // Recover the original record id when carried (F3); otherwise derive
            // a stable per-thread sequence id (backward-compat with old files).
            let id = carried_id.unwrap_or_else(|| format!("{thread_id}-r{seq}"));
            seq += 1;
            ThreadMessageRecord {
                id,
                thread_id: thread_id.to_owned(),
                turn_index,
                message,
                created_at,
            }
        })
        .collect()
}

/// Reconstruct [`TurnStateRecord`]s from rollout `TurnContext::TurnState` lines.
///
/// Field fidelity (F4/F6):
/// - `started_at` recovers the carried turn-start timestamp when present, else
///   falls back to the line write-time (backward-compat with old files).
/// - `input_messages_json`: when the rollout line carries a preserved raw blob
///   (`input_messages_raw`, set by the adapter when the typed list failed to
///   parse) AND the parsed list is empty, the raw blob is returned verbatim so
///   a malformed blob is recoverable instead of being silently emptied.
///   Otherwise the typed list is re-serialized (empty list → `None`).
fn replay_turn_states(thread_id: &str, lines: &[RolloutLine]) -> Vec<TurnStateRecord> {
    let mut out = Vec::new();
    for line in lines {
        let RolloutItem::TurnContext(TurnContextPayload::TurnState {
            turn_index,
            status,
            input_messages,
            tool_specs_json,
            llm_response_json,
            error,
            completed_at,
            started_at,
            input_messages_raw,
        }) = &line.item
        else {
            continue;
        };
        // F6: prefer the preserved raw blob when the typed list is empty (the
        // parse-failure path); else re-serialize the typed list.
        let input_messages_json = if input_messages.is_empty() {
            input_messages_raw.clone()
        } else {
            serde_json::to_string(input_messages).ok()
        };
        out.push(TurnStateRecord {
            thread_id: thread_id.to_owned(),
            turn_index: *turn_index,
            status: status.clone(),
            input_messages_json,
            tool_specs_json: tool_specs_json.clone(),
            llm_response_json: llm_response_json.clone(),
            error: error.clone(),
            // F4: recover the carried turn-start timestamp, else the line time.
            started_at: started_at.clone().unwrap_or_else(|| line.timestamp.clone()),
            completed_at: completed_at.clone(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use slab_agent::port::ThreadStatus;
    use slab_types::ConversationMessageContent;

    /// Minimal in-memory `AgentStorePort` mock used to verify the SQL fallback
    /// path (rollout file absent → delegate). Stores messages, turn items, and
    /// turn states so all three read fallback branches can be exercised. Also
    /// doubles as a [`RolloutIndex`] mock: an in-memory `backfill_status` map
    /// drives the Slice-5 read gate (a thread the adapter marks via
    /// `upsert_thread` becomes rollout-ready; an unseeded thread stays on SQL).
    struct MockStore {
        messages: std::sync::Mutex<Vec<ThreadMessageRecord>>,
        items: std::sync::Mutex<Vec<TurnItemRecord>>,
        states: std::sync::Mutex<Vec<TurnStateRecord>>,
        backfill: std::sync::Mutex<std::collections::HashMap<String, String>>,
        /// When true, `rollout_backfill_status` returns a synthetic SQLite error
        /// (G2 test: the read gate must resolve by file existence, not blindly
        /// fall back to SQL).
        index_error: std::sync::atomic::AtomicBool,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                messages: std::sync::Mutex::new(Vec::new()),
                items: std::sync::Mutex::new(Vec::new()),
                states: std::sync::Mutex::new(Vec::new()),
                backfill: std::sync::Mutex::new(std::collections::HashMap::new()),
                index_error: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl RolloutIndex for MockStore {
        async fn rollout_backfill_status(&self, thread_id: &str) -> sqlx::Result<Option<String>> {
            if self.index_error.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(sqlx::Error::Protocol("synthetic index error".into()));
            }
            Ok(self.backfill.lock().unwrap().get(thread_id).cloned())
        }

        async fn mark_rollout_session(
            &self,
            thread_id: &str,
            _session_id: &str,
            _file_path: &str,
            _last_turn_index: u32,
            _last_item_id: Option<&str>,
            _line_count: u32,
            backfill_status: &str,
        ) -> sqlx::Result<()> {
            self.backfill.lock().unwrap().insert(thread_id.to_owned(), backfill_status.to_owned());
            Ok(())
        }

        async fn list_thread_ids_for_backfill(&self) -> sqlx::Result<Vec<(String, String)>> {
            Ok(Vec::new())
        }

        async fn mark_backfill_state(
            &self,
            _thread_id: &str,
            _status: &str,
            _lines_written: u32,
            _error: Option<&str>,
        ) -> sqlx::Result<()> {
            Ok(())
        }

        // Mock threads never carry legacy SQL data, so `upsert_thread` always
        // marks them rollout-ready — matching the production new-thread path.
        async fn thread_has_legacy_data(&self, _thread_id: &str) -> sqlx::Result<bool> {
            Ok(false)
        }
    }

    #[async_trait]
    impl AgentStorePort for MockStore {
        async fn upsert_thread(&self, _snapshot: &ThreadSnapshot) -> Result<(), AgentError> {
            Ok(())
        }
        async fn get_thread(&self, _id: &str) -> Result<Option<ThreadSnapshot>, AgentError> {
            Ok(None)
        }
        async fn list_session_threads(
            &self,
            _session_id: &str,
        ) -> Result<Vec<ThreadSnapshot>, AgentError> {
            Ok(Vec::new())
        }
        async fn update_thread_status(
            &self,
            _id: &str,
            _status: ThreadStatus,
            _completion_text: Option<&str>,
        ) -> Result<(), AgentError> {
            Ok(())
        }
        async fn insert_tool_call(&self, _record: &ToolCallRecord) -> Result<(), AgentError> {
            Ok(())
        }
        async fn update_tool_call_status(
            &self,
            _id: &str,
            _status: ToolCallStatus,
        ) -> Result<(), AgentError> {
            Ok(())
        }
        async fn update_tool_call(
            &self,
            _id: &str,
            _output: Option<&str>,
            _status: ToolCallStatus,
            _completed_at: &str,
        ) -> Result<(), AgentError> {
            Ok(())
        }
        async fn insert_thread_message(
            &self,
            record: &ThreadMessageRecord,
        ) -> Result<(), AgentError> {
            self.messages.lock().unwrap().push(record.clone());
            Ok(())
        }
        async fn list_thread_messages(
            &self,
            _thread_id: &str,
        ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
            Ok(self.messages.lock().unwrap().clone())
        }
        async fn upsert_turn_state(&self, record: &TurnStateRecord) -> Result<(), AgentError> {
            let mut states = self.states.lock().unwrap();
            if let Some(existing) = states
                .iter_mut()
                .find(|s| s.thread_id == record.thread_id && s.turn_index == record.turn_index)
            {
                *existing = record.clone();
            } else {
                states.push(record.clone());
            }
            Ok(())
        }
        async fn list_turn_states(
            &self,
            thread_id: &str,
        ) -> Result<Vec<TurnStateRecord>, AgentError> {
            Ok(self
                .states
                .lock()
                .unwrap()
                .iter()
                .filter(|s| s.thread_id == thread_id)
                .cloned()
                .collect())
        }
        async fn insert_turn_item(&self, record: &TurnItemRecord) -> Result<(), AgentError> {
            self.items.lock().unwrap().push(record.clone());
            Ok(())
        }
        async fn list_turn_items(
            &self,
            thread_id: &str,
        ) -> Result<Vec<TurnItemRecord>, AgentError> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|i| i.thread_id == thread_id)
                .cloned()
                .collect())
        }
    }

    fn user_msg(text: &str) -> ConversationMessage {
        ConversationMessage {
            role: "user".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn snapshot(id: &str) -> ThreadSnapshot {
        ThreadSnapshot {
            id: id.to_owned(),
            session_id: "session-1".to_owned(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Running,
            role_name: None,
            config_json: "{}".to_owned(),
            completion_text: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            archived_at: None,
        }
    }

    /// Build an adapter backed by a [`MockStore`] (which impls both
    /// `AgentStorePort` and `RolloutIndex`). The same mock handle serves as both
    /// the SQL delegate and the index, mirroring how `SqlxStore` fills both
    /// roles in production. No trace dir (agent.debug off).
    fn adapter(mock: Arc<MockStore>, rollout: Arc<RolloutFileStore>) -> RolloutBackedAgentStore {
        RolloutBackedAgentStore::new(
            Arc::clone(&mock) as Arc<dyn AgentStorePort>,
            Arc::clone(&mock) as Arc<dyn RolloutIndex>,
            rollout,
            None,
        )
    }

    // (a) NEW thread: upsert_thread stamps SessionMeta, then writes/reads flow
    // through rollout (file_exists true, no SQL fallback).
    //
    // NOTE (F8): this test deliberately does NOT pre-flush the recorder before
    // reading — it mirrors production (no production caller pre-flushes). The
    // read methods flush unconditionally before the file_exists check (F2), and
    // the observer flushes at turn boundaries (F1), so the just-written items
    // are durable and the file is materialized in time.
    #[tokio::test]
    async fn new_thread_writes_and_reads_via_rollout() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        // No rollout file yet.
        assert!(!store.rollout.file_exists("t-new").await);

        // upsert_thread stamps the session header (creates the rollout file's
        // recorder; the file materializes on first write).
        store.upsert_thread(&snapshot("t-new")).await.expect("upsert");
        // Insert a message + a turn item through the adapter.
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: "m1".to_owned(),
                thread_id: "t-new".to_owned(),
                turn_index: 0,
                message: user_msg("hello"),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert message");
        let item_json = serde_json::to_string(&slab_agent::protocol::TurnItem::AgentMessage {
            id: "a1".to_owned(),
            text: "hi".to_owned(),
        })
        .unwrap();
        store
            .insert_turn_item(&TurnItemRecord {
                id: "a1".to_owned(),
                thread_id: "t-new".to_owned(),
                turn_index: 0,
                seq: 0,
                item_json,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert turn item");

        // No manual flush — the read methods flush before the file_exists check
        // (F2), so the lazy-materialized file is durable in time for the read.
        let messages = store.list_thread_messages("t-new").await.expect("list messages");
        assert_eq!(messages.len(), 1, "message read back from rollout");
        assert_eq!(messages[0].message.role, "user");
        assert_eq!(messages[0].turn_index, 0);
        assert_eq!(messages[0].thread_id, "t-new");

        let items = store.list_turn_items("t-new").await.expect("list items");
        assert_eq!(items.len(), 1, "turn item read back from rollout");
        assert_eq!(items[0].id, "a1");
        assert_eq!(items[0].turn_index, 0);

        // The reads flushed before the file_exists check (F2), so the rollout
        // file is now materialized → reads came from rollout, not MockStore.
        assert!(
            store.rollout.file_exists("t-new").await,
            "read-side flush materialized the rollout file"
        );
    }

    // (b) Fallback: when no rollout file exists, list_thread_messages delegates
    // to the SQL store.
    #[tokio::test]
    async fn unmigrated_thread_falls_back_to_sqlx() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(MockStore::new());
        // Seed the mock with a pre-existing (legacy) message.
        mock.insert_thread_message(&ThreadMessageRecord {
            id: "legacy-1".to_owned(),
            thread_id: "t-legacy".to_owned(),
            turn_index: 0,
            message: user_msg("old"),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        })
        .await
        .unwrap();

        let store = adapter(mock, Arc::clone(&rollout));
        // No rollout file for the legacy thread.
        assert!(!store.rollout.file_exists("t-legacy").await);

        let messages = store.list_thread_messages("t-legacy").await.expect("list");
        assert_eq!(messages.len(), 1, "delegated to SQL store");
        assert_eq!(messages[0].id, "legacy-1");
        assert_eq!(messages[0].message.role, "user");
    }

    // (c) delete_*_from routes to truncate; afterwards list_* is empty.
    #[tokio::test]
    async fn delete_from_routes_to_truncate() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-trunc")).await.unwrap();
        // Turn 0 + turn 1 messages.
        for turn in 0..2u32 {
            store
                .insert_thread_message(&ThreadMessageRecord {
                    id: format!("m{turn}"),
                    thread_id: "t-trunc".to_owned(),
                    turn_index: turn,
                    message: user_msg(&format!("t{turn}")),
                    created_at: "2026-01-01T00:00:00Z".to_owned(),
                })
                .await
                .unwrap();
        }

        // Drop turn 1+.
        store.delete_thread_messages_from("t-trunc", 1).await.expect("delete");
        // Idempotent: a second call is a no-op.
        store.delete_turn_states_from("t-trunc", 1).await.expect("delete again");

        let messages = store.list_thread_messages("t-trunc").await.expect("list");
        assert_eq!(messages.len(), 1, "only turn 0 survives");
        assert_eq!(messages[0].turn_index, 0);

        // Session header survives the truncation.
        assert!(store.rollout.read_session_meta("t-trunc").await.is_some());
    }

    // (d) F8 regression: a brand-new rollout-era thread's writes are readable
    // through the adapter with NO manual pre-flush (the false-green hole). The
    // read methods flush before the file_exists check (F2), so a thread that
    // has only ever been written through the adapter reads back correctly.
    #[tokio::test]
    async fn new_thread_reads_without_manual_flush() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-noflush")).await.expect("upsert");
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: "msg-9".to_owned(),
                thread_id: "t-noflush".to_owned(),
                turn_index: 0,
                message: user_msg("payload"),
                created_at: "2026-02-02T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert message");
        let item_json = serde_json::to_string(&slab_agent::protocol::TurnItem::AgentMessage {
            id: "it-9".to_owned(),
            text: "reply".to_owned(),
        })
        .unwrap();
        store
            .insert_turn_item(&TurnItemRecord {
                id: "it-9".to_owned(),
                thread_id: "t-noflush".to_owned(),
                turn_index: 0,
                seq: 0,
                item_json,
                created_at: "2026-02-02T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert turn item");

        // NO manual flush anywhere below — mirrors production.
        let messages = store.list_thread_messages("t-noflush").await.expect("list messages");
        assert_eq!(messages.len(), 1, "read hole fixed: readable without manual flush");
        // F3: the carried record id + created_at are recovered verbatim.
        assert_eq!(messages[0].id, "msg-9", "F3: original message id recovered");
        assert_eq!(
            messages[0].created_at, "2026-02-02T00:00:00Z",
            "F3: original created_at recovered"
        );

        let items = store.list_turn_items("t-noflush").await.expect("list items");
        assert_eq!(items.len(), 1, "turn items readable without manual flush");
        assert_eq!(items[0].id, "it-9");

        let states = store.list_turn_states("t-noflush").await.expect("list states");
        // No turn state written → empty (but the read must not error / fall
        // through to SQL, and the file must be materialized).
        assert!(states.is_empty(), "no turn states written");
        // The reads above proved the rollout file was materialized (else they
        // would have delegated to the empty MockStore).
        assert!(
            store.rollout.file_exists("t-noflush").await,
            "rollout file materialized by the read-side flush"
        );
    }

    // F7 (production path): a skipped compaction marker (status == "skipped",
    // emitted by slab-agent when a compaction is attempted but does not shrink)
    // must NOT orphan the conversation on the adapter's production read path
    // (list_thread_messages -> the local replay_messages). F7 first landed the
    // skipped-no-op only in the rollout crate's read_messages, which no
    // production caller uses; this test guards the adapter-local replay.
    #[tokio::test]
    async fn skipped_compacted_marker_does_not_orphan_via_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-skip")).await.expect("upsert");
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: "keep-1".to_owned(),
                thread_id: "t-skip".to_owned(),
                turn_index: 0,
                message: user_msg("keep1"),
                created_at: "2026-03-03T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert keep1");
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: "keep-2".to_owned(),
                thread_id: "t-skip".to_owned(),
                turn_index: 0,
                message: user_msg("keep2"),
                created_at: "2026-03-03T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert keep2");

        // Append a skipped Compacted marker exactly as the observer writes it
        // (status preserved from slab-agent's emit_compacted_skipped; empty
        // baseline because nothing was actually compacted).
        rollout
            .append(
                "t-skip",
                RolloutItem::Compacted(slab_agent_rollout::CompactedPayload {
                    thread_id: "t-skip".to_owned(),
                    compacted_messages: vec![],
                    removed_messages: 0,
                    output_tokens: 0,
                    status: "skipped".to_owned(),
                    turn_index: 0,
                }),
            )
            .await
            .expect("append skipped compacted");

        let messages = store.list_thread_messages("t-skip").await.expect("list messages");
        assert_eq!(
            messages.len(),
            2,
            "skipped compaction must NOT clear the baseline — conversation survives"
        );
        let texts: Vec<&str> = messages
            .iter()
            .map(|m| match &m.message.content {
                slab_types::ConversationMessageContent::Text(t) => t.as_str(),
                _ => "",
            })
            .collect();
        assert_eq!(texts, vec!["keep1", "keep2"]);
    }

    // (e) F9: list_turn_items + list_turn_states fall back to SQL when no
    // rollout file exists. Covers the SQL-fallback branch for all three read
    // methods (messages are covered by (b)).
    #[tokio::test]
    async fn unmigrated_thread_falls_back_to_sqlx_for_items_and_states() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(MockStore::new());
        // Seed the mock with a pre-existing (legacy) item + state.
        mock.insert_turn_item(&TurnItemRecord {
            id: "legacy-item".to_owned(),
            thread_id: "t-legacy2".to_owned(),
            turn_index: 3,
            seq: 1,
            item_json: serde_json::to_string(&slab_agent::protocol::TurnItem::AgentMessage {
                id: "legacy-item".to_owned(),
                text: "old-item".to_owned(),
            })
            .unwrap(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        })
        .await
        .unwrap();
        mock.upsert_turn_state(&TurnStateRecord {
            thread_id: "t-legacy2".to_owned(),
            turn_index: 3,
            status: "completed".to_owned(),
            input_messages_json: Some("[]".to_owned()),
            tool_specs_json: None,
            llm_response_json: None,
            error: None,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            completed_at: Some("2026-01-01T00:01:00Z".to_owned()),
        })
        .await
        .unwrap();

        let store = adapter(mock, Arc::clone(&rollout));
        assert!(!store.rollout.file_exists("t-legacy2").await);

        let items = store.list_turn_items("t-legacy2").await.expect("list items");
        assert_eq!(items.len(), 1, "items delegated to SQL store");
        assert_eq!(items[0].id, "legacy-item");
        assert_eq!(items[0].turn_index, 3);

        let states = store.list_turn_states("t-legacy2").await.expect("list states");
        assert_eq!(states.len(), 1, "states delegated to SQL store");
        assert_eq!(states[0].turn_index, 3);
        assert_eq!(states[0].status, "completed");
    }

    // (f) F9: rollout-first for items + states when the file exists (mirrors (a)
    // for the two read methods not previously covered end-to-end).
    #[tokio::test]
    async fn migrated_thread_reads_items_and_states_from_rollout() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        // Pre-seed the SQL mock so a fall-through would be detectable.
        let mock = Arc::new(MockStore::new());
        mock.insert_turn_item(&TurnItemRecord {
            id: "SQL-SHOULD-NOT-APPEAR".to_owned(),
            thread_id: "t-migrated".to_owned(),
            turn_index: 0,
            seq: 0,
            item_json: "{}".to_owned(),
            created_at: "x".to_owned(),
        })
        .await
        .unwrap();
        let store = adapter(mock, Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-migrated")).await.expect("upsert");
        let item_json = serde_json::to_string(&slab_agent::protocol::TurnItem::AgentMessage {
            id: "rl-item".to_owned(),
            text: "from-rollout".to_owned(),
        })
        .unwrap();
        store
            .insert_turn_item(&TurnItemRecord {
                id: "rl-item".to_owned(),
                thread_id: "t-migrated".to_owned(),
                turn_index: 0,
                seq: 0,
                item_json,
                created_at: "2026-03-03T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert item");

        let items = store.list_turn_items("t-migrated").await.expect("list items");
        assert_eq!(items.len(), 1, "rollout-first, not SQL");
        assert_eq!(items[0].id, "rl-item");
        // The SQL-seeded item did NOT leak through.
        assert!(
            !items.iter().any(|i| i.id == "SQL-SHOULD-NOT-APPEAR"),
            "rollout-first must not fall through to SQL"
        );
    }

    // (g) F9: upsert_turn_state -> list_turn_states round-trip through the
    // adapter asserts full field fidelity (including F4 started_at and the F6
    // raw-blob recovery path).
    #[tokio::test]
    async fn upsert_turn_state_round_trips_with_fidelity() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-state")).await.expect("upsert");
        let input = vec![user_msg("in")];
        let input_json = serde_json::to_string(&input).unwrap();
        store
            .upsert_turn_state(&TurnStateRecord {
                thread_id: "t-state".to_owned(),
                turn_index: 7,
                status: "completed".to_owned(),
                input_messages_json: Some(input_json.clone()),
                tool_specs_json: Some("[\"spec\"]".to_owned()),
                llm_response_json: Some("{\"r\":1}".to_owned()),
                error: None,
                started_at: "2026-04-04T00:00:00Z".to_owned(),
                completed_at: Some("2026-04-04T00:00:05Z".to_owned()),
            })
            .await
            .expect("upsert turn state");

        let states = store.list_turn_states("t-state").await.expect("list states");
        assert_eq!(states.len(), 1);
        let s = &states[0];
        assert_eq!(s.turn_index, 7);
        assert_eq!(s.status, "completed");
        assert_eq!(s.input_messages_json.as_deref(), Some(input_json.as_str()));
        assert_eq!(s.tool_specs_json.as_deref(), Some("[\"spec\"]"));
        assert_eq!(s.llm_response_json.as_deref(), Some("{\"r\":1}"));
        assert!(s.error.is_none());
        // F4: real started_at recovered (NOT the line write-time).
        assert_eq!(s.started_at, "2026-04-04T00:00:00Z", "F4: started_at recovered");
        assert_eq!(s.completed_at.as_deref(), Some("2026-04-04T00:00:05Z"));
    }

    // (h) F6: a malformed input_messages_json blob is preserved as raw on the
    // rollout and recovered verbatim by list_turn_states (not silently emptied).
    #[tokio::test]
    async fn malformed_input_messages_blob_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-bad")).await.expect("upsert");
        let malformed = "{not valid json";
        store
            .upsert_turn_state(&TurnStateRecord {
                thread_id: "t-bad".to_owned(),
                turn_index: 1,
                status: "running".to_owned(),
                input_messages_json: Some(malformed.to_owned()),
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                started_at: "2026-05-05T00:00:00Z".to_owned(),
                completed_at: None,
            })
            .await
            .expect("upsert turn state");

        let states = store.list_turn_states("t-bad").await.expect("list states");
        assert_eq!(states.len(), 1);
        // F6: the raw blob is recoverable (NOT lost to unwrap_or_default).
        assert_eq!(
            states[0].input_messages_json.as_deref(),
            Some(malformed),
            "F6: malformed blob preserved verbatim, not emptied"
        );
    }

    // G2: when the rollout_session_index lookup errors, the read gate must
    // resolve by FILE EXISTENCE — a rollout-native thread (file present, SQL
    // empty) reads from rollout, NOT an empty SQL fallback. Pre-fix the gate
    // returned false on any index error, serving empty history for a new
    // thread on a transient SQLite hiccup.
    #[tokio::test]
    async fn index_lookup_error_resolves_by_file_existence() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(MockStore::new());
        // Make the index lookup fail for every thread.
        mock.index_error.store(true, std::sync::atomic::Ordering::Relaxed);

        let store = adapter(Arc::clone(&mock), Arc::clone(&rollout));

        // (a) A rollout-native thread: a write materializes the rollout file
        // (lazily, on flush). The index lookup errors, but the file exists →
        // rollout-ready → the read comes from rollout (not the empty MockStore).
        store.upsert_thread(&snapshot("t-native")).await.expect("upsert");
        store
            .insert_thread_message(&ThreadMessageRecord {
                id: "native-msg".to_owned(),
                thread_id: "t-native".to_owned(),
                turn_index: 0,
                message: user_msg("from-rollout"),
                created_at: "2026-06-06T00:00:00Z".to_owned(),
            })
            .await
            .expect("insert");
        // The MockStore's SQL history is empty for t-native (writes go to
        // rollout), so a fallback would return []. list_thread_messages flushes
        // internally (materializing the file) BEFORE the gate check, so the
        // file_exists gate resolves to true.
        let messages = store.list_thread_messages("t-native").await.expect("list");
        assert_eq!(
            messages.len(),
            1,
            "index error + file present → read from rollout, not empty SQL"
        );
        assert_eq!(messages[0].id, "native-msg");

        // (b) A legacy thread: no rollout file. The index lookup errors, the
        // file does NOT exist → fall back to SQL (the legacy data lives there).
        mock.insert_thread_message(&ThreadMessageRecord {
            id: "legacy-msg".to_owned(),
            thread_id: "t-legacy-idx".to_owned(),
            turn_index: 0,
            message: user_msg("from-sql"),
            created_at: "2026-06-06T00:00:00Z".to_owned(),
        })
        .await
        .unwrap();
        assert!(!store.rollout.file_exists("t-legacy-idx").await);
        let messages = store.list_thread_messages("t-legacy-idx").await.expect("list");
        assert_eq!(
            messages.len(),
            1,
            "index error + no file → fall back to SQL for the legacy thread"
        );
        assert_eq!(messages[0].id, "legacy-msg");
    }

    // Slice 11b: a ROOT thread (no parent) upserted while a trace dir is
    // configured gets trace_path = Some(trace_dir) on its SessionMeta; a CHILD
    // thread (with a parent) gets None and is expected to correlate back to its
    // root thread's bundle via root_thread_id.
    #[tokio::test]
    async fn root_thread_session_meta_carries_trace_path_child_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let trace_dir = std::path::PathBuf::from("/some/trace/dir");
        let mock = Arc::new(MockStore::new());
        let store = RolloutBackedAgentStore::new(
            Arc::clone(&mock) as Arc<dyn AgentStorePort>,
            Arc::clone(&mock) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            Some(trace_dir.clone()),
        );

        // Root thread: no parent → trace_path stamped.
        store.upsert_thread(&snapshot("t-root")).await.expect("upsert root");
        let root_meta = rollout.read_session_meta("t-root").await.expect("root meta");
        assert_eq!(
            root_meta.trace_path.as_deref(),
            Some("/some/trace/dir"),
            "root thread SessionMeta carries the trace dir"
        );

        // Child thread: parent_id set → trace_path None (correlates via root).
        let mut child = snapshot("t-child");
        child.parent_id = Some("t-root".to_owned());
        store.upsert_thread(&child).await.expect("upsert child");
        let child_meta = rollout.read_session_meta("t-child").await.expect("child meta");
        assert!(
            child_meta.trace_path.is_none(),
            "child thread SessionMeta carries no trace_path (inherits via root_thread_id)"
        );
    }

    // Slice 11b: when no trace dir is configured (agent.debug off), even a root
    // thread gets trace_path = None — the coordination is opt-in.
    #[tokio::test]
    async fn no_trace_dir_means_no_trace_path_even_for_root() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let store = adapter(Arc::new(MockStore::new()), Arc::clone(&rollout));

        store.upsert_thread(&snapshot("t-root2")).await.expect("upsert");
        let meta = rollout.read_session_meta("t-root2").await.expect("meta");
        assert!(
            meta.trace_path.is_none(),
            "no trace dir configured → root thread trace_path stays None"
        );
    }
}
