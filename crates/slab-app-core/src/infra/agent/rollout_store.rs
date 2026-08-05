//! Rollout-backed [`AgentStorePort`] adapter — the true source for agent
//! conversation and turn state.
//!
//! Slice 4 wired this adapter as the **only** `AgentStorePort` impl in the
//! agent runtime. Slice E made the rollout JSONL the SOLE source: the legacy
//! conversation + audit tables were dropped, the SQL read fallback + the startup
//! backfill were removed, and the `AgentStorePort` trait was slimmed to the
//! surface slab-agent actually calls in production (thread metadata + the three
//! conversation/turn methods). Turn-state / turn-item READS moved off the
//! slab-agent trait entirely (slab-agent does not call them) and onto the
//! app-core-internal [`RolloutConversationReader`] trait, implemented below.
//!
//! # Routing (Slice E — rollout is the only source)
//! - **Thread metadata** (`upsert_thread` / `get_thread` / `list_session_threads` /
//!   `update_thread_status` / `archive_thread`) → always `SqlxStore`
//!   (`agent_threads` stays the metadata truth source). `upsert_thread` also
//!   stamps the rollout `SessionMeta` header on first upsert and marks the
//!   thread rollout-native (`backfill_status = "completed"`) at creation.
//!   `list_session_threads[_filtered]` are the EXCEPTION: they read the DB
//!   metadata list but then drop TRUE GHOSTS — threads the index claims are
//!   `backfill_status = "completed"` whose rollout file has gone missing — and,
//!   when the DB is unavailable, fall back to a best-effort filesystem scan
//!   (Slice D2a). See [`RolloutBackedAgentStore::list_session_threads_filtered`].
//! - **Conversation writes/reads** (`insert_thread_message` /
//!   `upsert_turn_state` / `list_thread_messages`) → rollout only
//!   (`TurnContext` / `TurnItem`). A missing rollout file replays to an empty
//!   history (the de-facto behavior of a brand-new thread before its first
//!   append).
//! - **Turn-state / turn-item reads** (`list_turn_states` / `list_turn_items`,
//!   on [`RolloutConversationReader`]) → rollout replay only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use slab_agent::error::AgentError;
use slab_agent::port::{
    AgentStorePort, ThreadListFilter, ThreadMessageRecord, ThreadSnapshot, TurnItemRecord,
    TurnStateRecord,
};
use slab_agent_rollout::{
    RolloutFileStore, RolloutItem, RolloutLine, RolloutStore, SessionMeta, TurnContextPayload,
    read_rollout_lines,
};
// `SessionMeta` is re-exported above for `build_session_meta` and the D2a
// filesystem fallback; `read_first_line_session_meta` is private to the rollout
// crate, so the fallback reuses `RolloutFileStore::list_all_session_metas`.
use slab_types::ConversationMessage;

use crate::domain::services::agent::RolloutConversationReader;
use crate::infra::db::repository::rollout_index::RolloutIndex;

/// `AgentStorePort` impl that backs reads/writes with the rollout JSONL true
/// source, delegating only thread metadata to the SQL store.
pub struct RolloutBackedAgentStore {
    /// SQL delegate for thread metadata (`agent_threads` is the metadata truth
    /// source). Slice E dropped the legacy conversation/audit tables + the SQL
    /// read fallback, so this delegate no longer serves conversation data.
    sqlx: Arc<dyn AgentStorePort>,
    /// app-core-internal handle over the `rollout_session_index` table — backs
    /// the D2a list ghost-gate + the new-thread `backfill_status = "completed"`
    /// mark. NOT on `AgentStorePort` (slab-agent stays pure).
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

    /// The SHARED production read path for the LLM-visible conversation, used by
    /// BOTH the agent runtime (`list_thread_messages` below) AND the agent-memory
    /// phase1 pipeline (`memory::build_phase1_input`).
    ///
    /// Slice E: rollout is the ONLY source, so this flushes the recorder and
    /// replays the rollout file. A missing rollout file (a brand-new thread
    /// before its first append) replays to an empty history — the de-facto
    /// behavior. The memory pipeline stamps `rollout_path` from
    /// `resolve_path`, which yields the on-disk path whether or not the file
    /// has materialized yet.
    pub(crate) async fn read_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        let lines = read_rollout_lines(&self.rollout.resolve_path(thread_id));
        Ok(replay_messages(thread_id, &lines))
    }

    // ── Slice D2a: list dual-source scheduling (ghost exclusion + DB fallback) ──

    /// Drop TRUE GHOSTS from a DB-sourced thread list, logging each removal as a
    /// best-effort read-repair signal. See [`ThreadReadability::classify`] for
    /// the gate. Non-ghosts are returned in their original (DB) order.
    ///
    /// **Read-repair policy**: this is a READ path; the only safe, side-effect-
    /// free repair is to keep the ghost INVISIBLE to the user (filter it out).
    /// Actively deleting the orphaned `agent_threads` row is deliberately NOT
    /// done here — a transient condition (the rollout file on a momentarily-
    /// unmounted drive, or an index row wrongly marked completed by a bug) would
    /// cause permanent metadata loss from a read. The structured `warn!` is the
    /// signal a future janitor / the D2b backfill-lease task can scrape to delete
    /// orphans once the condition is confirmed persistent. `AgentStorePort` has
    /// no `delete_thread` today, and adding one would widen the slab-agent
    /// contract for a destructive op better owned by an explicit cleanup task.
    ///
    /// **Batching (H1+M2)**: this is the list HOT path (sidebar), so it must NOT
    /// do K sequential synchronous filesystem lookups. It makes TWO calls total:
    ///
    /// 1. ONE `list_all_session_metas()` filesystem scan → a `HashSet` of
    ///    thread ids whose rollout file exists on disk (replaces K per-thread
    ///    `lookup_path` calls, each of which does its own date-tree scan).
    /// 2. ONE batch `rollout_backfill_progress_for(...)` DB query → a map of
    ///    `(backfill_status, line_count)` (replaces K per-thread
    ///    `rollout_backfill_status` queries).
    ///
    /// On a batch-DB-lookup ERROR the whole list is kept (no thread is orphaned
    /// on a transient DB hiccup) — mirrors the single-thread Err branch.
    async fn exclude_true_ghosts(&self, snapshots: Vec<ThreadSnapshot>) -> Vec<ThreadSnapshot> {
        if snapshots.is_empty() {
            return snapshots;
        }
        // (M2) ONE filesystem scan instead of K per-thread `lookup_path` calls.
        let file_present: std::collections::HashSet<String> =
            self.rollout.list_all_session_metas().into_iter().map(|m| m.thread_id).collect();
        // (H1+M2) ONE batch DB query: `(backfill_status, line_count)` per thread.
        let ids: Vec<String> = snapshots.iter().map(|s| s.id.clone()).collect();
        let progress = match self.index.rollout_backfill_progress_for(&ids).await {
            Ok(map) => map,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "list_session_threads: batch rollout_session_index lookup failed; \
                     keeping all threads (no thread is orphaned on a transient DB error)",
                );
                return snapshots;
            }
        };
        let mut readable = Vec::with_capacity(snapshots.len());
        for snap in snapshots {
            match ThreadReadability::classify(&snap.id, &file_present, &progress) {
                ThreadReadability::Readable => readable.push(snap),
                ThreadReadability::TrueGhost => {
                    tracing::warn!(
                        thread_id = %snap.id,
                        session_id = %snap.session_id,
                        "list_session_threads: dropping true ghost \
                         (backfill_status=completed, line_count>0, but rollout file missing); \
                         metadata row left in place for a janitor to reclaim",
                    );
                }
            }
        }
        readable
    }

    /// Best-effort DB-unavailable fallback: reconstruct a thread listing purely
    /// from the rollout true source (the `SessionMeta` header of every rollout
    /// file), with degraded fields. Used ONLY when the metadata DB cannot be
    /// queried (see `is_db_unavailable`).
    ///
    /// **Field degradation** (acceptable — the DB is down, so the authoritative
    /// fields are unavailable): `status` defaults to `Running` (the "active"
    /// default; there is no `Unknown` variant), `updated_at` reuses `started_at`
    /// (fallback has no updated_at), `depth` is `0` (a root thread's depth;
    /// child threads are filtered out — see below), `archived_at` is `None`
    /// (`SessionMeta` carries no archived info, so the `include_archived` flag
    /// cannot be honored — degraded mode returns all). The DB-side filters that
    /// CAN be honored in degraded mode (`session_id`, cursor, limit) are applied
    /// here; `parent_id IS NULL` is mirrored by keeping only root threads.
    fn list_threads_from_filesystem(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Vec<ThreadSnapshot> {
        let mut snapshots: Vec<ThreadSnapshot> = self
            .rollout
            .list_all_session_metas()
            .into_iter()
            // Mirror the DB query: only ROOT threads (parent_id IS NULL) for the
            // requested session.
            .filter(|m| m.parent_id.is_none() && m.session_id == session_id)
            .map(session_meta_to_degraded_snapshot)
            .collect();
        // Sort newest-first by started_at (the DB orders by updated_at DESC;
        // started_at is the closest available proxy in degraded mode).
        snapshots.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        // Cursor: started_at approximates updated_at (degraded — no updated_at).
        if let Some(before) = &filter.before_updated_at {
            snapshots.retain(|s| s.updated_at.as_str() < before.as_str());
        }
        // Limit (after cursor + sort, matching the DB's LIMIT semantics).
        if let Some(limit) = filter.limit
            && let Ok(n) = usize::try_from(limit)
        {
            snapshots.truncate(n);
        }
        snapshots
    }
}

/// Extra rows the list path asks the DB for beyond the client `limit` so ghost
/// exclusion (which runs AFTER the DB LIMIT) does not under-fill a page and
/// trip body.rs's `len < limit` → "no next page" heuristic (M1). The pad is
/// `min(limit, THREAD_LIST_OVERFETCH_PAD)` per page — enough to absorb a
/// realistic cluster of ghosts without 2x-ing an already-large query.
const THREAD_LIST_OVERFETCH_PAD: usize = 64;

/// Readability verdict for the list-path ghost gate (Slice D2a).
enum ThreadReadability {
    /// Keep the thread: rollout file present, OR a newborn native thread
    /// (`completed` + `line_count == 0`, empty but healthy), OR an index row
    /// that does not claim `completed` with `line_count > 0`.
    Readable,
    /// Drop the thread: rollout file absent AND index claims `completed` AND
    /// `line_count > 0` — data was supposedly migrated into the now-missing
    /// file, so the rollout replay can no longer serve it.
    TrueGhost,
}

impl ThreadReadability {
    /// The list-path readability gate. Sync + allocation-free so
    /// [`RolloutBackedAgentStore::exclude_true_ghosts`] can call it in a tight
    /// loop over the precomputed fs-scan set + batch DB progress map (no
    /// per-thread async fs / DB calls — see M2).
    ///
    /// - **Readable** (keep):
    ///   - the rollout file EXISTS on disk (rollout-native or already
    ///     backfilled — rollout is the true source), OR
    ///   - the rollout file is absent but the index is NOT `completed` (an
    ///     un-backfilled LEGACY thread whose conversation is still served by the
    ///     SQL read fallback), OR
    ///   - the rollout file is absent, the index IS `completed`, but
    ///     `line_count == 0` — a **NEWBORN** native thread (H1).
    ///     `upsert_thread` stamps `completed` + `line_count = 0` at creation,
    ///     BEFORE the recorder's first append lazily materializes the rollout
    ///     file. Dropping it would hide a brand-new session from the sidebar
    ///     for the whole create→first-append window (and permanently if a crash
    ///     precedes the first append). It is empty but healthy.
    /// - **TrueGhost** (drop): the rollout file is absent AND the index claims
    ///   `completed` AND `line_count > 0` — the thread ONCE HAD data migrated
    ///   into a file that has since gone missing. This is the only real loss.
    ///
    /// A thread with no index row (`progress` miss) resolves to `(None, 0)` →
    /// Readable, matching the un-backfilled-legacy branch.
    fn classify(
        thread_id: &str,
        file_present: &std::collections::HashSet<String>,
        progress: &std::collections::HashMap<String, (Option<String>, i64)>,
    ) -> ThreadReadability {
        // (1) rollout file present → readable (rollout is the true source).
        if file_present.contains(thread_id) {
            return ThreadReadability::Readable;
        }
        // (2) file absent. Defer to the index: status + line_count together
        //     distinguish a newborn (empty, healthy) from a true ghost (data
        //     lost). A miss → (None, 0) → un-backfilled legacy → readable.
        let (status, line_count) = progress.get(thread_id).cloned().unwrap_or((None, 0));
        if status.as_deref() == Some("completed") && line_count > 0 {
            ThreadReadability::TrueGhost
        } else {
            ThreadReadability::Readable
        }
    }
}

/// Whether an [`AgentError`] from the list path is a "DB-unavailable" class
/// error (SQLite lock / corruption / connection failure / missing relation) for
/// which the rollout-filesystem fallback is appropriate. Any other error (e.g. a
/// row-deserialization bug) is a real defect and must propagate.
///
/// The SqlxStore flattens `sqlx::Error` to `AgentError::Store(e.to_string())`,
/// so this inspects the message text. The curated signal set covers the SQLite
/// + connection failures that mean "the DB cannot serve this read right now".
fn is_db_unavailable(error: &AgentError) -> bool {
    let AgentError::Store(msg) = error else {
        return false;
    };
    let lower = msg.to_ascii_lowercase();
    const SIGNALS: &[&str] = &[
        "database is locked",
        "database table is locked",
        "database disk image is malformed",
        "unable to open database file",
        "disk i/o error",
        "no such table",
        "no such database",
        "server closed the connection",
        "connection refused",
        "broken pipe",
        "the database file is locked",
    ];
    SIGNALS.iter().any(|s| lower.contains(s))
}

/// Reconstruct a degraded [`ThreadSnapshot`] from a rollout [`SessionMeta`] for
/// the DB-unavailable filesystem fallback. See
/// [`RolloutBackedAgentStore::list_threads_from_filesystem`] for the field-
/// degradation rationale.
fn session_meta_to_degraded_snapshot(meta: SessionMeta) -> ThreadSnapshot {
    ThreadSnapshot {
        id: meta.thread_id,
        session_id: meta.session_id,
        parent_id: meta.parent_id,
        depth: 0,
        status: slab_agent::port::ThreadStatus::Running,
        role_name: meta.role_name,
        config_json: serde_json::to_string(&meta.config_json).unwrap_or_else(|_| "{}".to_owned()),
        completion_text: None,
        created_at: meta.started_at.clone(),
        // No updated_at in SessionMeta — reuse started_at (degraded).
        updated_at: meta.started_at,
        // No archived info in SessionMeta (degraded — treated as not archived).
        archived_at: None,
    }
}

/// Build a rollout [`SessionMeta`] header from a [`ThreadSnapshot`], applying
/// the SINGLE canonical root-vs-child `trace_path` rule: a ROOT thread (no
/// `parent_id`) stamped with the per-root-thread trace BUNDLE directory when
/// agent debugging is on; a CHILD thread always carries `None` (it correlates
/// back to its root thread's trace bundle via `root_thread_id`).
///
/// `trace_dir` here is the configured agent-trace BASE directory (the legacy
/// log dir). The stamped `trace_path` points at the deterministic per-root
/// bundle directory `<trace_dir>/agent_trace/trace-<root_thread_id>-<root_thread_id>`
/// — the EXACT directory the live `BundleAgentTraceSink` writes into, so a
/// diagnostic can jump from the rollout file straight to the bundle. The
/// formula lives in `slab_agent_tracing::bundle_dir_for_root_thread` and is
/// shared with the sink so the two cannot drift.
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
    // Slice 11b/0: stamp trace_path ONLY on a root thread (no parent). It points
    // at the per-root-thread bundle dir (NOT the legacy shared log dir) so it
    // matches the live sink's output. Child threads correlate back via
    // root_thread_id, so they carry None and inherit the pointer.
    let trace_path = if snapshot.parent_id.is_none() {
        trace_dir.map(|dir| {
            slab_agent_tracing::bundle_dir_for_root_thread(dir, &snapshot.id)
                .to_string_lossy()
                .into_owned()
        })
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
        // Slice E: every thread is rollout-native now (the legacy conversation
        // tables + the startup backfill are gone), so unconditionally stamp the
        // index row `backfill_status = "completed"` + `line_count = 0` (the
        // recorder's first append materializes the file + bumps line_count
        // later). Pre-Slice-E this probed `thread_has_legacy_data` to avoid
        // orphaning a legacy prefix; that probe + the legacy tables are gone.
        //
        // The rollout file materializes (lazily) at the date-partitioned
        // path_for_new location derived from snapshot.created_at. If a file
        // already exists somewhere in the lookup chain (e.g. a migrated flat
        // file), use its real on-disk path; otherwise stamp the path the
        // recorder WILL write so the DB points where the bytes land.
        let file_path = self
            .rollout
            .lookup_path(&snapshot.id)
            .unwrap_or_else(|| self.rollout.path_for_new(&snapshot.id, &snapshot.created_at))
            .to_string_lossy()
            .into_owned();
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
                "failed to stamp rollout_session_index",
            );
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
        // The unfiltered variant routes through the filtered dual-source path so
        // it gets the SAME ghost exclusion + DB-unavailable fallback. The filter
        // mirrors the ORIGINAL unfiltered SQL semantics exactly: no limit, no
        // cursor, and ARCHIVED THREADS INCLUDED (the original SQL had no
        // `archived_at IS NULL` filter — e.g. `restore_session` relies on the
        // newest root thread regardless of archived status). Only the ghost
        // exclusion is new.
        self.list_session_threads_filtered(
            session_id,
            &ThreadListFilter { include_archived: true, ..Default::default() },
        )
        .await
    }

    async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, AgentError> {
        // DB is the AUTHORITATIVE metadata source (limit/cursor/archived honored
        // there). On a DB-unavailable error we degrade to a filesystem scan; on
        // any other error we propagate. The fallback is attempted on the FIRST
        // page read only (see the over-fetch loop below).
        //
        // (M1) Over-fetch when a limit is set: ghost exclusion happens AFTER
        // the DB LIMIT, so a page that drops K ghosts returns `limit - K` rows.
        // body.rs treats `snapshots.len() < limit` as "no next page", which
        // would hide older reachable threads sitting behind a ghost cluster
        // (e.g. with [t4, t3ghost, t2, t1] and limit=2 the DB returns [t4, t3],
        // t3 is dropped → [t4], len 1 < 2 → next_cursor=None → t2/t1 never
        // fetched). To keep the body.rs next_cursor contract honest we request
        // a PADDED limit (`limit + pad`) from the DB, drop ghosts, then truncate
        // back to `limit`. `pad = limit` (2x) tolerates up to `limit` ghosts in
        // a single client page — far more than the realistic density (a ghost
        // is a genuinely-missing rollout file). With no limit (unfiltered /
        // restore_session) there is no pagination concern, so a single read +
        // ghost filter suffices.
        let target = match filter.limit {
            Some(target) => usize::try_from(target).unwrap_or(usize::MAX),
            None => {
                let rows = self.sqlx.list_session_threads_filtered(session_id, filter).await;
                let rows = match rows {
                    Ok(rows) => rows,
                    Err(error) if is_db_unavailable(&error) => {
                        tracing::warn!(
                            session_id = %session_id,
                            error = %error,
                            "list_session_threads: DB unavailable; \
                             falling back to rollout filesystem scan",
                        );
                        return Ok(self.list_threads_from_filesystem(session_id, filter));
                    }
                    Err(error) => return Err(error),
                };
                return Ok(self.exclude_true_ghosts(rows).await);
            }
        };
        if target == 0 {
            return Ok(Vec::new());
        }
        // Pad each DB page so a handful of ghosts within the window don't force
        // an under-full client page. Capped at a sane ceiling so a huge `limit`
        // (e.g. 10_000) does not 2x an already-large query unboundedly.
        let pad = target.min(THREAD_LIST_OVERFETCH_PAD);
        let padded_limit = u32::try_from(target + pad).unwrap_or(u32::MAX);
        let page_filter = ThreadListFilter {
            limit: Some(padded_limit),
            before_updated_at: filter.before_updated_at.clone(),
            include_archived: filter.include_archived,
        };
        let rows = match self.sqlx.list_session_threads_filtered(session_id, &page_filter).await {
            Ok(rows) => rows,
            Err(error) if is_db_unavailable(&error) => {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "list_session_threads: DB unavailable; \
                     falling back to rollout filesystem scan",
                );
                // The filesystem fallback honors the ORIGINAL (un-padded) filter
                // so the client sees `target` rows at most, matching the DB path.
                return Ok(self.list_threads_from_filesystem(session_id, filter));
            }
            Err(error) => return Err(error),
        };
        let mut readable = self.exclude_true_ghosts(rows).await;
        readable.truncate(target);
        Ok(readable)
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

    // ── Conversation writes → rollout (sole source since Slice E) ─────────

    async fn insert_thread_message(&self, record: &ThreadMessageRecord) -> Result<(), AgentError> {
        self.rollout
            .append(
                &record.thread_id,
                RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                    turn_index: record.turn_index,
                    message: record.message.clone(),
                    // F3: carry the record id + created_at through the rollout so
                    // replay recovers the original values (frontends use message.id
                    // as a React key; without these the replay substitutes a
                    // synthetic `{thread_id}-r{seq}` id + the line write-time).
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
        // being silently emptied.
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

    // ── Reads → rollout only (sole source since Slice E) ───────────────────
    //
    // The flush below runs UNCONDITIONALLY before the read (F2): the recorder
    // is lazy (it only writes on Persist/Shutdown/Truncate), so without this
    // flush a freshly-written thread's pending items are not durable and the
    // rollout read would miss them. The flush is a no-op when no recorder
    // exists. A missing rollout file replays to an empty history.

    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, AgentError> {
        // Delegates to the shared read path (also used by the memory pipeline)
        // so the runtime and the memory model observe the SAME conversation.
        self.read_thread_messages(thread_id).await
    }
}

// ── Slice E: turn-state / turn-item reads live on the app-core-internal
//    `RolloutConversationReader` trait (slab-agent does not call them; only
//    `HarnessService::thread/resume` does). Rollout replay is the only source.

#[async_trait]
impl RolloutConversationReader for RolloutBackedAgentStore {
    async fn list_turn_items(&self, thread_id: &str) -> Result<Vec<TurnItemRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        Ok(self.rollout.read_turn_items(thread_id).await)
    }

    async fn list_turn_states(&self, thread_id: &str) -> Result<Vec<TurnStateRecord>, AgentError> {
        let _ = self.rollout.flush(thread_id).await;
        let lines = read_rollout_lines(&self.rollout.resolve_path(thread_id));
        Ok(replay_turn_states(thread_id, &lines))
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
///
/// `pub(crate)` so the agent-memory pipeline can replay the LLM-visible
/// conversation (with faithful `created_at`) from the SAME projection the
/// production read path uses — the two cannot diverge.
pub(crate) fn replay_messages(thread_id: &str, lines: &[RolloutLine]) -> Vec<ThreadMessageRecord> {
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

    /// Minimal in-memory `AgentStorePort` mock backing the rollout-native read
    /// path tests. Stores messages and turn states so the adapter's rollout
    /// replay can be exercised without a real SqlxStore. Also doubles as a
    /// [`RolloutIndex`] mock: an in-memory `backfill_status` / `line_count`
    /// map drives the D2a list ghost-gate (the list-path readability classifier
    /// runs against this mock + the real `RolloutFileStore`).
    struct MockStore {
        messages: std::sync::Mutex<Vec<ThreadMessageRecord>>,
        states: std::sync::Mutex<Vec<TurnStateRecord>>,
        backfill: std::sync::Mutex<std::collections::HashMap<String, (String, i64)>>,
        /// When true, `rollout_backfill_status` returns a synthetic SQLite error
        /// (G2 test: the read gate must resolve by file existence, not blindly
        /// fall back to SQL).
        index_error: std::sync::atomic::AtomicBool,
        /// Seeded thread listing returned by `list_session_threads[_filtered]`
        /// (filtered by `session_id`). Used by the D2a fallback tests to drive
        /// the list path through the mock without a real SqlxStore.
        threads: std::sync::Mutex<Vec<ThreadSnapshot>>,
        /// When set, `list_session_threads[_filtered]` returns
        /// `AgentError::Store(this message)` instead of the seeded listing —
        /// drives the D2a DB-unavailable fallback + non-DB-error propagation
        /// tests. Stored as a message string (AgentError is not Clone).
        list_error: std::sync::Mutex<Option<String>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                messages: std::sync::Mutex::new(Vec::new()),
                states: std::sync::Mutex::new(Vec::new()),
                backfill: std::sync::Mutex::new(std::collections::HashMap::new()), // id → (status, line_count)
                index_error: std::sync::atomic::AtomicBool::new(false),
                threads: std::sync::Mutex::new(Vec::new()),
                list_error: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl RolloutIndex for MockStore {
        async fn rollout_backfill_status(&self, thread_id: &str) -> sqlx::Result<Option<String>> {
            if self.index_error.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(sqlx::Error::Protocol("synthetic index error".into()));
            }
            Ok(self.backfill.lock().unwrap().get(thread_id).map(|(status, _)| status.clone()))
        }

        async fn rollout_backfill_progress_for(
            &self,
            thread_ids: &[String],
        ) -> sqlx::Result<std::collections::HashMap<String, (Option<String>, i64)>> {
            if self.index_error.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(sqlx::Error::Protocol("synthetic index error".into()));
            }
            let map = self.backfill.lock().unwrap();
            Ok(thread_ids
                .iter()
                .filter_map(|id| {
                    map.get(id).map(|(status, lc)| (id.clone(), (Some(status.clone()), *lc)))
                })
                .collect())
        }

        async fn mark_rollout_session(
            &self,
            thread_id: &str,
            _session_id: &str,
            _file_path: &str,
            _last_turn_index: u32,
            _last_item_id: Option<&str>,
            line_count: u32,
            backfill_status: &str,
        ) -> sqlx::Result<()> {
            self.backfill
                .lock()
                .unwrap()
                .insert(thread_id.to_owned(), (backfill_status.to_owned(), i64::from(line_count)));
            Ok(())
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
            session_id: &str,
        ) -> Result<Vec<ThreadSnapshot>, AgentError> {
            // D2a: when a list error is armed, surface it (drives the fallback /
            // propagation tests through the real adapter, not a bypass).
            if let Some(msg) = self.list_error.lock().unwrap().clone() {
                return Err(AgentError::Store(msg));
            }
            Ok(self
                .threads
                .lock()
                .unwrap()
                .iter()
                .filter(|t| t.session_id == session_id)
                .cloned()
                .collect())
        }
        async fn update_thread_status(
            &self,
            _id: &str,
            _status: ThreadStatus,
            _completion_text: Option<&str>,
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
    }

    // Slice E: list_turn_states / list_turn_items moved off `AgentStorePort`
    // onto the app-core-internal `RolloutConversationReader` trait. The adapter
    // (`RolloutBackedAgentStore`) implements it above; `MockStore` is never cast
    // as `Arc<dyn RolloutConversationReader>` (production wires the adapter, not
    // the SQL mock), so no impl is needed here.

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
        // Seed a TurnItem directly through the rollout (Slice E removed the
        // adapter's `insert_turn_item`; production writes TurnItems via the
        // rollout persistence observer, not the store trait).
        rollout
            .append(
                "t-new",
                RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                    id: "a1".to_owned(),
                    text: "hi".to_owned(),
                }),
            )
            .await
            .expect("append turn item");

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

        // The reads flushed before the file_exists check (F2), so the rollout
        // file is now materialized → reads came from rollout, not MockStore.
        assert!(
            store.rollout.file_exists("t-new").await,
            "read-side flush materialized the rollout file"
        );
    }
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
        // Seed a TurnItem directly through the rollout (Slice E removed the
        // adapter's `insert_turn_item`).
        rollout
            .append(
                "t-noflush",
                RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                    id: "it-9".to_owned(),
                    text: "reply".to_owned(),
                }),
            )
            .await
            .expect("append turn item");

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
        // No turn state written → empty (the reader replays an empty TurnState set).
        assert!(states.is_empty(), "no turn states written");
        // The reads above proved the rollout file was materialized (else the
        // message read would have replayed empty).
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
    // Slice 11b/0: a ROOT thread (no parent) upserted while a trace dir is
    // configured gets trace_path = Some(<per-root-thread bundle dir>) on its
    // SessionMeta; a CHILD thread (with a parent) gets None and is expected to
    // correlate back to its root thread's bundle via root_thread_id. The bundle
    // dir is the deterministic path the live BundleAgentTraceSink writes into.
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

        // Root thread: no parent → trace_path stamped at the per-root bundle dir.
        store.upsert_thread(&snapshot("t-root")).await.expect("upsert root");
        let root_meta = rollout.read_session_meta("t-root").await.expect("root meta");
        let expected = slab_agent_tracing::bundle_dir_for_root_thread(&trace_dir, "t-root")
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            root_meta.trace_path.as_deref(),
            Some(expected.as_str()),
            "root thread SessionMeta carries the per-root-thread bundle dir"
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

    // Slice 0 consistency: the `trace_path` that `build_session_meta` stamps on
    // a root thread's SessionMeta MUST be the exact directory the live
    // `BundleAgentTraceSink` writes its bundle into. Both use the shared
    // `bundle_dir_for_root_thread` formula; this test pins them together so a
    // diagnostic can jump from the rollout file straight to a populated bundle.
    #[tokio::test]
    async fn build_session_meta_trace_path_matches_live_sink_bundle_dir() {
        use slab_agent_tracing::{
            AgentTraceContext, AgentTraceEvent, AgentTraceSink, BundleAgentTraceSink,
            bundle_dir_for_root_thread,
        };

        let trace_root = tempfile::tempdir().expect("trace root");
        let trace_dir = trace_root.path().to_path_buf();
        let root_thread_id = "consistency-root";

        // (1) Drive the LIVE bundle sink through the production record path so a
        // bundle is actually materialized on disk.
        let sink = BundleAgentTraceSink::new(trace_dir.clone());
        let ctx = AgentTraceContext::new("session-1")
            .with_thread(root_thread_id)
            .with_root_thread_id(root_thread_id)
            .with_trace_dir(trace_dir.clone());
        sink.record(
            &ctx,
            AgentTraceEvent::new(
                "slab-agent",
                "agent_llm_request",
                serde_json::json!({ "model": "x", "messages": [] }),
            ),
        );

        // (2) Compute the SessionMeta trace_path via the canonical helper used
        // by upsert_thread / fork / compact.
        let snap = snapshot(root_thread_id);
        let meta = build_session_meta(&snap, Some(trace_dir.as_path()));

        // (3) The stamped path points at the bundle dir that now exists on disk.
        let stamped = meta.trace_path.expect("root thread stamps a trace_path");
        let expected_dir = bundle_dir_for_root_thread(&trace_dir, root_thread_id);
        assert_eq!(
            stamped,
            expected_dir.to_string_lossy().into_owned(),
            "SessionMeta.trace_path must equal the deterministic bundle dir"
        );

        // (4) The bundle the sink wrote is AT that path (manifest + trace.jsonl).
        assert!(
            expected_dir.join(slab_agent_tracing::MANIFEST_FILE).is_file(),
            "manifest exists at the stamped trace_path"
        );
        assert!(
            expected_dir.join(slab_agent_tracing::TRACE_FILE).is_file(),
            "trace.jsonl exists at the stamped trace_path"
        );

        // (5) A child of this root carries None and correlates via root_thread_id.
        let mut child = snapshot("consistency-child");
        child.parent_id = Some(root_thread_id.to_owned());
        let child_meta = build_session_meta(&child, Some(trace_dir.as_path()));
        assert!(child_meta.trace_path.is_none(), "child carries no trace_path");
    }

    // ── Slice D2a: list dual-source scheduling (ghost exclusion + DB fallback) ──
    //
    // These tests exercise the REAL SqlxStore (sqlite::memory:, impls both
    // AgentStorePort + RolloutIndex) + a REAL RolloutFileStore over a tempdir,
    // seeding DB rows / index rows / rollout files INDEPENDENTLY so each
    // readability class (rollout-native, un-backfilled legacy, true ghost) is
    // constructed deliberately. No store mock on the main path — a false-green
    // here (e.g. the gate treating every file-absent thread as a ghost) would
    // hide the legacy-history-loss regression.

    /// A thread snapshot for `id` in `session`, at the given `updated_at`.
    /// `created_at` mirrors `updated_at` (the cursor/limit tests rely on the
    /// DB `updated_at DESC` ordering, which SqlxStore preserves verbatim).
    fn snap_at(id: &str, session: &str, updated_at: &str) -> ThreadSnapshot {
        ThreadSnapshot {
            id: id.to_owned(),
            session_id: session.to_owned(),
            parent_id: None,
            depth: 0,
            status: ThreadStatus::Running,
            role_name: None,
            config_json: "{}".to_owned(),
            completion_text: None,
            created_at: updated_at.to_owned(),
            updated_at: updated_at.to_owned(),
            archived_at: None,
        }
    }

    /// Build an adapter over a REAL in-memory SqlxStore (migrated, impls both
    /// AgentStorePort + RolloutIndex) and the given RolloutFileStore. Returns
    /// the concrete `Arc<AnyStore>` so a test can seed DB rows / index rows
    /// directly (the Arc derefs to SqlxStore, which impls both traits) AND
    /// access `.pool` to insert the `chat_sessions` FK parent a thread row needs.
    async fn real_adapter(
        rollout: Arc<RolloutFileStore>,
    ) -> (RolloutBackedAgentStore, Arc<crate::infra::db::AnyStore>) {
        let sqlx: Arc<crate::infra::db::AnyStore> =
            Arc::new(crate::test_support::migrated_test_store().await);
        let store = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx) as Arc<dyn RolloutIndex>,
            rollout,
            None,
        );
        (store, sqlx)
    }

    /// Insert the `chat_sessions` parent row a `agent_threads` FK requires.
    /// `agent_threads.session_id REFERENCES chat_sessions(id) ON DELETE CASCADE`,
    /// so an `upsert_thread` fails with a FK violation until the session exists.
    async fn seed_session(sqlx: &crate::infra::db::AnyStore, session_id: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO chat_sessions (id, name, created_at, updated_at) \
             VALUES (?1, '', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(session_id)
        .execute(&sqlx.pool)
        .await
        .expect("seed chat_sessions row");
    }

    // The core dual-source test: one thread of EACH readability class in the
    // same session. The list must keep rollout-native + un-backfilled legacy
    // (pending AND no-index-row) and drop ONLY the true ghost.
    //
    // Mutation guards (each must flip a below assertion):
    //  - Revert `exclude_true_ghosts` to pass-through (return all snapshots) →
    //    t-ghost appears → the `!contains("t-ghost")` assertion fails.
    //  - Make `ThreadReadability::classify` drop ANY thread whose rollout file
    //    is absent (the legacy-history-loss bug) → t-legacy-pending +
    //    t-legacy-none disappear → the `contains` assertions for them fail.
    #[tokio::test]
    async fn list_keeps_legacy_and_native_drops_only_true_ghost() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-d2a";
        seed_session(&sqlx, session).await;

        // (1) TRUE GHOST: DB row + index claims completed + line_count>0 (it
        // ONCE HAD migrated data) + NO rollout file (the file has since gone
        // missing). line_count>0 is what distinguishes a real ghost from a
        // NEWBORN native thread (completed + line_count=0 + file not yet
        // materialized) — see `newborn_native_thread_visible_before_first_append`.
        sqlx.upsert_thread(&snap_at("t-ghost", session, "2026-08-01T00:00:00Z")).await.unwrap();
        sqlx.mark_rollout_session(
            "t-ghost",
            session,
            "/ghost/absent.jsonl",
            0,
            None,
            5,
            "completed",
        )
        .await
        .unwrap();
        // Deliberately DO NOT create the rollout file → it is a ghost.

        // (2) LEGACY (pending): DB row + index pending + NO rollout file.
        sqlx.upsert_thread(&snap_at("t-legacy-pending", session, "2026-08-02T00:00:00Z"))
            .await
            .unwrap();
        sqlx.mark_rollout_session(
            "t-legacy-pending",
            session,
            "/legacy/absent.jsonl",
            0,
            None,
            0,
            "pending",
        )
        .await
        .unwrap();

        // (3) LEGACY (no index row at all): DB row only, no index, no rollout file.
        sqlx.upsert_thread(&snap_at("t-legacy-none", session, "2026-08-03T00:00:00Z"))
            .await
            .unwrap();

        // (4) ROLLOUT-NATIVE: a rollout file exists (created via the store), DB
        // row present, index completed. The file existing makes it readable
        // regardless of the index status.
        rollout.create_session(SessionMeta {
            thread_id: "t-native".to_owned(),
            session_id: session.to_owned(),
            parent_id: None,
            started_at: "2026-08-04T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        rollout
            .append(
                "t-native",
                RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                    id: "n1".to_owned(),
                    text: "native".to_owned(),
                }),
            )
            .await
            .unwrap();
        rollout.flush("t-native").await.unwrap();
        sqlx.upsert_thread(&snap_at("t-native", session, "2026-08-04T00:00:00Z")).await.unwrap();
        sqlx.mark_rollout_session(
            "t-native",
            session,
            dir.path().join("x").to_string_lossy().as_ref(),
            0,
            None,
            0,
            "completed",
        )
        .await
        .unwrap();
        // Sanity: the native rollout file is discoverable by lookup_path.
        assert!(rollout.lookup_path("t-native").is_some());
        // Sanity: the ghost rollout file is NOT discoverable.
        assert!(rollout.lookup_path("t-ghost").is_none());

        let listed = store.list_session_threads(session).await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();

        // The ghost is dropped; the rest survive.
        assert!(!ids.contains(&"t-ghost".to_owned()), "true ghost excluded: {ids:?}");
        assert!(ids.contains(&"t-native".to_owned()), "rollout-native kept: {ids:?}");
        assert!(
            ids.contains(&"t-legacy-pending".to_owned()),
            "legacy (pending) kept — SQL fallback reads it: {ids:?}",
        );
        assert!(
            ids.contains(&"t-legacy-none".to_owned()),
            "legacy (no index row) kept — SQL fallback reads it: {ids:?}",
        );
        assert_eq!(ids.len(), 3, "exactly the three readable threads");
    }

    // Pin the ghost-gate NUANCE: a thread whose rollout file is absent is a
    // ghost ONLY when the index says `completed`. The other backfill states
    // (in_progress / failed) — and the index-lookup error path — must keep the
    // thread (legacy SQL fallback). This is the regression guard for the most
    // dangerous false-green: a gate that drops every file-absent thread would
    // pass a naive "ghost excluded" test while silently deleting legacy history.
    //
    // Mutation: make `ThreadReadability::classify` return TrueGhost for any
    // absent file (ignoring the status) → t-in-progress, t-failed, t-idx-err
    // all vanish → their `contains` assertions fail.
    #[tokio::test]
    async fn list_ghost_gate_respects_completed_status_only() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-gate";
        seed_session(&sqlx, session).await;

        for (id, status) in [
            ("t-completed", "completed"), // ghost (file absent + completed + line_count>0)
            ("t-in-progress", "in_progress"), // kept (legacy fallback)
            ("t-failed", "failed"),       // kept (legacy fallback)
        ] {
            sqlx.upsert_thread(&snap_at(id, session, "2026-08-01T00:00:00Z")).await.unwrap();
            // line_count>0 for the completed ghost so it is a REAL ghost (had
            // data) and not a newborn (completed + line_count=0).
            let line_count: u32 = if status == "completed" { 5 } else { 0 };
            sqlx.mark_rollout_session(id, session, "/absent.jsonl", 0, None, line_count, status)
                .await
                .unwrap();
        }

        // The index-lookup Err branch is covered separately by the mock-backed
        // test below (the real SqlxStore has no error knob). Here we pin the
        // three deterministic status branches.

        let listed = store.list_session_threads(session).await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();

        assert!(!ids.contains(&"t-completed".to_owned()), "completed + absent = ghost: {ids:?}");
        assert!(
            ids.contains(&"t-in-progress".to_owned()),
            "in_progress + absent = legacy (kept): {ids:?}",
        );
        assert!(ids.contains(&"t-failed".to_owned()), "failed + absent = legacy (kept): {ids:?}");
    }

    // Index-lookup ERROR resolves to Readable (keep) — never orphan history on a
    // transient DB hiccup. Uses the MockStore (arms `index_error`) since the
    // real SqlxStore has no error knob; the gate logic under test is the adapter
    // `is_thread_readable`, which is mock-agnostic.
    #[tokio::test]
    async fn list_index_lookup_error_keeps_thread_not_orphans_it() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(MockStore::new());
        // Arm the index to error for every lookup.
        mock.index_error.store(true, std::sync::atomic::Ordering::Relaxed);
        // Seed a thread the DB (mock) lists — no rollout file. snapshot() uses
        // session_id "session-1", which the mock's list filters by.
        mock.threads.lock().unwrap().push(snapshot("t-idx-err"));

        let store = adapter(Arc::clone(&mock), Arc::clone(&rollout));
        let listed = store.list_session_threads("session-1").await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        // Mutation: make the Err arm return TrueGhost → t-idx-err vanishes → fails.
        assert!(
            ids.contains(&"t-idx-err".to_owned()),
            "index-lookup error must keep the thread (legacy fallback), not orphan it: {ids:?}",
        );
    }

    // Cursor + limit still work under dual-source scheduling. Uses ALL
    // rollout-native threads so ghost exclusion does not perturb the counts
    // (every thread is readable). The DB honors limit/cursor; the adapter only
    // filters ghosts afterwards, so the counts here are exact.
    #[tokio::test]
    async fn list_cursor_and_limit_still_correct_under_dual_source() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-page";
        seed_session(&sqlx, session).await;

        // Three rollout-native threads with distinct updated_at (DB orders DESC).
        for (id, ts) in [
            ("t-old", "2026-08-01T00:00:00Z"),
            ("t-mid", "2026-08-02T00:00:00Z"),
            ("t-new", "2026-08-03T00:00:00Z"),
        ] {
            rollout.create_session(SessionMeta {
                thread_id: id.to_owned(),
                session_id: session.to_owned(),
                parent_id: None,
                started_at: ts.to_owned(),
                config_json: serde_json::json!({}),
                rollout_version: SessionMeta::CURRENT_VERSION,
                role_name: None,
                trace_path: None,
            });
            rollout
                .append(
                    id,
                    RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                        id: format!("{id}-m"),
                        text: "x".to_owned(),
                    }),
                )
                .await
                .unwrap();
            rollout.flush(id).await.unwrap();
            sqlx.upsert_thread(&snap_at(id, session, ts)).await.unwrap();
        }

        // Limit = 2 → newest two (t-new, t-mid).
        let limited = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter { limit: Some(2), ..Default::default() },
            )
            .await
            .expect("list limited");
        let ids: Vec<&str> = limited.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t-new", "t-mid"], "limit honored, newest first");

        // Cursor before t-mid → only t-old is strictly older.
        let cursor = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter {
                    before_updated_at: Some("2026-08-02T00:00:00Z".to_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("list cursor");
        let ids: Vec<&str> = cursor.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t-old"], "cursor returns only strictly-older threads");
    }

    // DB-unavailable fallback: when the metadata DB returns a "database is
    // locked"-class error, list degrades to a filesystem scan over the rollout
    // true source. A DB mock arms the error (the real SqlxStore has no knob);
    // the rollout files + the adapter fallback path under test are REAL.
    //
    // Mutation: make `is_db_unavailable` return false for everything (propagate
    // all errors) → list returns Err → the `expect("list")` panics.
    // Mutation: make the fallback return an empty vec (skip the scan) → the
    // `t-fb-native` assertion fails.
    #[tokio::test]
    async fn list_db_unavailable_falls_back_to_filesystem_scan() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let session = "s-fb";

        // Seed a REAL rollout file the fallback scan must discover.
        rollout.create_session(SessionMeta {
            thread_id: "t-fb-native".to_owned(),
            session_id: session.to_owned(),
            parent_id: None,
            started_at: "2026-08-05T00:00:00Z".to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        rollout
            .append(
                "t-fb-native",
                RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                    id: "fb1".to_owned(),
                    text: "fb".to_owned(),
                }),
            )
            .await
            .unwrap();
        rollout.flush("t-fb-native").await.unwrap();

        // Arm the DB mock to fail the list with a DB-unavailable error.
        let mock = Arc::new(MockStore::new());
        *mock.list_error.lock().unwrap() = Some("database is locked".to_owned());
        let store = adapter(Arc::clone(&mock), Arc::clone(&rollout));

        let listed = store.list_session_threads(session).await.expect("fallback list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(
            ids.contains(&"t-fb-native".to_owned()),
            "DB-unavailable fallback scanned the rollout file: {ids:?}",
        );
        // Degraded fields: status defaults to Running, updated_at = started_at.
        let fb = listed.iter().find(|t| t.id == "t-fb-native").expect("found");
        assert_eq!(fb.status, ThreadStatus::Running, "degraded status defaults to active");
        assert_eq!(fb.updated_at, "2026-08-05T00:00:00Z", "updated_at degraded to started_at");
        assert_eq!(fb.session_id, session);
    }

    // Non-DB errors propagate (the fallback must NOT paper over real bugs). A
    // deserialization-style Store error is not in the DB-unavailable signal set.
    //
    // Mutation: make `is_db_unavailable` return true for everything → the
    // fallback runs and returns Ok → the `is_err` assertion fails.
    #[tokio::test]
    async fn list_non_db_error_propagates_not_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let mock = Arc::new(MockStore::new());
        // A Store error that is NOT a DB-unavailability signal.
        *mock.list_error.lock().unwrap() = Some("depth conversion overflow in row".to_owned());
        let store = adapter(Arc::clone(&mock), Arc::clone(&rollout));

        let result = store.list_session_threads("s-prop").await;
        assert!(result.is_err(), "non-DB error must propagate, not fall back");
        match result {
            Err(AgentError::Store(msg)) => {
                assert!(msg.contains("depth conversion"), "original error preserved: {msg}");
            }
            other => panic!("expected Store error, got {other:?}"),
        }
    }

    // The UNFILTERED list_session_threads routes through the same dual-source
    // path (ghost exclusion). A ghost seeded for the unfiltered call must also
    // be dropped — guards against a bypass where only the filtered variant is
    // gated.
    #[tokio::test]
    async fn list_unfiltered_also_excludes_true_ghost() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-unfilt";
        seed_session(&sqlx, session).await;

        sqlx.upsert_thread(&snap_at("t-ghost2", session, "2026-08-01T00:00:00Z")).await.unwrap();
        sqlx.mark_rollout_session("t-ghost2", session, "/absent.jsonl", 0, None, 5, "completed")
            .await
            .unwrap();
        sqlx.upsert_thread(&snap_at("t-real2", session, "2026-08-02T00:00:00Z")).await.unwrap();
        sqlx.mark_rollout_session("t-real2", session, "/absent.jsonl", 0, None, 0, "pending")
            .await
            .unwrap();

        let listed = store.list_session_threads(session).await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(!ids.contains(&"t-ghost2".to_owned()), "ghost excluded on unfiltered path");
        assert!(ids.contains(&"t-real2".to_owned()), "legacy kept on unfiltered path");
    }

    // ── H1: newborn native thread must NOT be mistaken for a true ghost ──────

    // The H1 regression: `upsert_thread` for a brand-new native thread (no
    // legacy SQL data) calls `create_session` (lazy — the rollout file is NOT
    // materialized until the first append/flush) and then marks the index
    // `backfill_status = "completed"` with `line_count = 0`. For the whole
    // create→first-append window the rollout file is absent, so a gate that
    // drops ANY `completed + file-absent` thread would HIDE THE NEW SESSION
    // from the sidebar (and permanently if a crash precedes the first append).
    // The `line_count > 0` part of the gate is what tells a newborn
    // (`line_count == 0`, empty but healthy) from a true ghost (had data, lost
    // it). This test drives the REAL production upsert path so the index mark
    // is exactly what production writes.
    //
    // Mutation: drop the `line_count > 0` condition in `ThreadReadability::classify`
    // (so `completed + file-absent` is always TrueGhost) → the newborn vanishes
    // → the `contains("t-newborn")` assertion fails.
    #[tokio::test]
    async fn newborn_native_thread_visible_before_first_append() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-newborn";
        seed_session(&sqlx, session).await;

        // Production path: upsert a brand-new thread (no legacy data). This
        // creates the recorder (file NOT yet on disk) AND marks the index
        // completed + line_count=0.
        store
            .upsert_thread(&snap_at("t-newborn", session, "2026-08-01T00:00:00Z"))
            .await
            .expect("upsert newborn");

        // Sanity: the rollout file is genuinely NOT materialized yet (the
        // create→first-append window the gate must tolerate).
        assert!(
            rollout.lookup_path("t-newborn").is_none(),
            "newborn rollout file not materialized until first append"
        );
        // Sanity: the index row is exactly the newborn profile (completed, line_count=0).
        let progress =
            sqlx.rollout_backfill_progress_for(&["t-newborn".to_owned()]).await.expect("progress");
        assert_eq!(
            progress.get("t-newborn"),
            Some(&(Some("completed".to_owned()), 0)),
            "newborn index row: completed + line_count=0",
        );

        // The newborn MUST appear in the list despite its rollout file being
        // absent — it is empty but healthy, not a ghost.
        let listed = store.list_session_threads(session).await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(
            ids.contains(&"t-newborn".to_owned()),
            "newborn native thread visible before first append: {ids:?}",
        );
    }

    // Bracket the OTHER side of the H1 line_count gate: a `completed` thread
    // whose rollout file is gone AND that once had data (`line_count > 0`) IS a
    // true ghost and must be dropped. Together with the newborn test above,
    // these pin the `line_count` discriminator from both directions.
    //
    // Mutation: make `classify` treat `completed + line_count>0 + absent` as
    // Readable (drop the TrueGhost arm) → t-real-ghost appears → the
    // `!contains` assertion fails.
    #[tokio::test]
    async fn true_ghost_with_line_count_still_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-ghost-lc";
        seed_session(&sqlx, session).await;

        // A REAL ghost: completed + line_count>0 (had migrated data) + no file.
        sqlx.upsert_thread(&snap_at("t-real-ghost", session, "2026-08-01T00:00:00Z"))
            .await
            .unwrap();
        sqlx.mark_rollout_session("t-real-ghost", session, "/gone.jsonl", 3, None, 7, "completed")
            .await
            .unwrap();
        // No rollout file created → it is a true ghost.

        let listed = store.list_session_threads(session).await.expect("list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(
            !ids.contains(&"t-real-ghost".to_owned()),
            "completed + line_count>0 + file-absent is a true ghost, dropped: {ids:?}",
        );
    }

    // ── M1: pagination must skip ghosts to fill the page ────────────────────

    // The M1 regression: ghost exclusion runs AFTER the DB LIMIT, so a page
    // that drops a ghost returns fewer than `limit` rows. body.rs treats
    // `len < limit` as "no next page", hiding older reachable threads behind a
    // ghost. The adapter over-fetches (`limit + pad`) so a handful of ghosts in
    // the window don't under-fill the page. With [t4, t3ghost, t2, t1] and
    // limit=2, page1 must be [t4, t2] (ghost t3 skipped, page filled) and a
    // cursor follow-up must reach t1.
    //
    // Mutation: set the over-fetch pad to 0 (`let pad = 0;` in
    // `list_session_threads_filtered`) → page1 DB read is [t4, t3], t3 dropped →
    // [t4], len 1 < 2 → the `eq!(page1, [t4, t2])` assertion fails AND t2/t1
    // become unreachable on page1.
    #[tokio::test]
    async fn list_pagination_skips_ghosts_to_fill_page() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let (store, sqlx) = real_adapter(Arc::clone(&rollout)).await;
        let session = "s-page-ghost";
        seed_session(&sqlx, session).await;

        // Four root threads, distinct updated_at (newest → oldest). t3 is a
        // ghost (completed + line_count>0 + no rollout file).
        let seeds: [(&str, &str, u32, &str); 4] = [
            ("t1", "2026-08-01T00:00:00Z", 0, "pending"),
            ("t2", "2026-08-02T00:00:00Z", 0, "pending"),
            ("t3", "2026-08-03T00:00:00Z", 4, "completed"), // ghost
            ("t4", "2026-08-04T00:00:00Z", 0, "pending"),
        ];
        for (id, ts, lc, status) in seeds {
            sqlx.upsert_thread(&snap_at(id, session, ts)).await.unwrap();
            sqlx.mark_rollout_session(id, session, "/absent.jsonl", 0, None, lc, status)
                .await
                .unwrap();
        }

        // Page 1 (limit 2): t4 + t2 (skip ghost t3, fill the page).
        let page1 = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter { limit: Some(2), ..Default::default() },
            )
            .await
            .expect("page1");
        let ids: Vec<&str> = page1.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t4", "t2"], "page1 fills the limit by skipping the ghost");
        assert!(
            !ids.contains(&"t3"),
            "ghost t3 must NOT appear on page1 even though it is within the DB window",
        );

        // Page 2 (cursor before the oldest page1 row = t2): reaches t1.
        let page2 = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter {
                    limit: Some(2),
                    before_updated_at: Some("2026-08-02T00:00:00Z".to_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("page2");
        let ids: Vec<&str> = page2.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t1"], "page2 reaches the oldest thread past the ghost");
    }

    // ── M3: DB-unavailable filesystem fallback coverage ─────────────────────

    /// Seed a REAL rollout file (header + one line + flush) for `thread_id` so
    /// `list_all_session_metas` discovers it during the DB-unavailable fallback.
    /// The append + flush MATERIALIZE the file on disk (create_session alone is
    /// lazy); without them the fallback scan would see nothing.
    async fn seed_rollout_file(
        rollout: &RolloutFileStore,
        thread_id: &str,
        session_id: &str,
        parent_id: Option<String>,
        started_at: &str,
    ) {
        rollout.create_session(SessionMeta {
            thread_id: thread_id.to_owned(),
            session_id: session_id.to_owned(),
            parent_id,
            started_at: started_at.to_owned(),
            config_json: serde_json::json!({}),
            rollout_version: SessionMeta::CURRENT_VERSION,
            role_name: None,
            trace_path: None,
        });
        rollout
            .append(
                thread_id,
                RolloutItem::TurnItem(slab_agent::protocol::TurnItem::AgentMessage {
                    id: format!("{thread_id}-m"),
                    text: "x".to_owned(),
                }),
            )
            .await
            .unwrap();
        rollout.flush(thread_id).await.unwrap();
    }

    /// Helper: drive the list through the DB-unavailable fallback by arming a
    /// "database is locked" error on a MockStore. The rollout files + the
    /// adapter fallback path under test are REAL.
    fn fallback_store(rollout: Arc<RolloutFileStore>) -> (RolloutBackedAgentStore, Arc<MockStore>) {
        let mock = Arc::new(MockStore::new());
        *mock.list_error.lock().unwrap() = Some("database is locked".to_owned());
        (adapter(Arc::clone(&mock), rollout), mock)
    }

    // Fallback cursor + limit: the retain (cursor) and truncate (limit) logic
    // must run AFTER the newest-first sort. Seed 3 files in one session with
    // distinct started_at (the fallback's updated_at proxy), then assert a
    // cursor excludes newer ones and a limit caps the count.
    //
    // Mutation: drop the `truncate` (limit) in `list_threads_from_filesystem`
    // → the limit assertion fails. Mutation: drop the `retain` (cursor) → the
    // cursor assertion fails.
    #[tokio::test]
    async fn fallback_honors_cursor_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let session = "s-fb-page";
        // Seed 3 files, newest → oldest.
        for (id, ts) in [
            ("fb-old", "2026-08-01T00:00:00Z"),
            ("fb-mid", "2026-08-02T00:00:00Z"),
            ("fb-new", "2026-08-03T00:00:00Z"),
        ] {
            seed_rollout_file(&rollout, id, session, None, ts).await;
        }
        let (store, _mock) = fallback_store(Arc::clone(&rollout));

        // Limit = 2 → newest two (fb-new, fb-mid), newest-first.
        let limited = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter { limit: Some(2), ..Default::default() },
            )
            .await
            .expect("fallback limited");
        let ids: Vec<&str> = limited.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["fb-new", "fb-mid"], "fallback limit honored, newest first");

        // Cursor before fb-mid → only fb-old is strictly older.
        let cursor = store
            .list_session_threads_filtered(
                session,
                &ThreadListFilter {
                    before_updated_at: Some("2026-08-02T00:00:00Z".to_owned()),
                    ..Default::default()
                },
            )
            .await
            .expect("fallback cursor");
        let ids: Vec<&str> = cursor.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["fb-old"], "fallback cursor returns only strictly-older");
    }

    // Fallback filters by session: listing session A MUST NOT surface rollout
    // files belonging to session B. Guards against an unfiltered cross-session
    // scan returning another session's threads.
    //
    // Mutation: drop the `m.session_id == session_id` filter in
    // `list_threads_from_filesystem` → session B's thread leaks in → the
    // `!contains` assertion fails.
    #[tokio::test]
    async fn fallback_filters_by_session() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        seed_rollout_file(&rollout, "fb-a", "session-a", None, "2026-08-01T00:00:00Z").await;
        seed_rollout_file(&rollout, "fb-b", "session-b", None, "2026-08-02T00:00:00Z").await;
        let (store, _mock) = fallback_store(Arc::clone(&rollout));

        let listed = store.list_session_threads("session-a").await.expect("fallback list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(ids.contains(&"fb-a".to_owned()), "session-a thread present: {ids:?}");
        assert!(
            !ids.contains(&"fb-b".to_owned()),
            "session-b thread must NOT leak into session-a list: {ids:?}",
        );
    }

    // Fallback excludes CHILD threads: the DB list query filters
    // `parent_id IS NULL` (root threads only). The fallback mirrors this so a
    // child thread (parent_id = Some(root)) is NOT returned as a top-level
    // entry. Guards against a child appearing as its own sidebar row.
    //
    // Mutation: drop the `m.parent_id.is_none()` filter in
    // `list_threads_from_filesystem` → the child leaks in → the
    // `!contains` assertion fails.
    #[tokio::test]
    async fn fallback_excludes_child_threads() {
        let dir = tempfile::tempdir().unwrap();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        let session = "s-fb-child";
        seed_rollout_file(&rollout, "fb-root", session, None, "2026-08-01T00:00:00Z").await;
        seed_rollout_file(
            &rollout,
            "fb-child",
            session,
            Some("fb-root".to_owned()),
            "2026-08-02T00:00:00Z",
        )
        .await;
        let (store, _mock) = fallback_store(Arc::clone(&rollout));

        let listed = store.list_session_threads(session).await.expect("fallback list");
        let ids: Vec<String> = listed.iter().map(|t| t.id.clone()).collect();
        assert!(ids.contains(&"fb-root".to_owned()), "root thread present: {ids:?}");
        assert!(
            !ids.contains(&"fb-child".to_owned()),
            "child thread must NOT appear as a top-level row: {ids:?}",
        );
    }
}
