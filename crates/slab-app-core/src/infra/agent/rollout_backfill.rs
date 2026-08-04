//! Slice-5 backfill: copy legacy SQL conversation rows into the rollout JSONL.
//!
//! At startup [`backfill_all_threads`] is spawned fire-and-forget (see
//! `bootstrap`). For each thread not yet `backfill_status = "completed"` in
//! `rollout_session_index`, it reads the legacy three tables
//! (`agent_thread_messages` / `agent_turn_states` / `agent_turn_items`),
//! replays them into the rollout file as `TurnContext` (`MessageAppend` /
//! `TurnState`) + `TurnItem` lines, flushes, then flips the index to
//! `completed`. New threads are stamped `completed` at creation by the rollout
//! adapter, so they are skipped here.
//!
//! # Faithful migration (per thread)
//! The three tables map onto the rollout line types so the adapter reads
//! (`replay_messages` / `read_turn_items` / `replay_turn_states`) reconstruct
//! the SAME data the SQL store returns:
//! - `agent_thread_messages` → `TurnContext::MessageAppend`, carrying the
//!   original record `id` + `created_at` (F3) so `replay_messages` recovers
//!   them verbatim.
//! - `agent_turn_states` → `TurnContext::TurnState` with **typed
//!   `input_messages` EMPTY** and the raw blob in `input_messages_raw` (F6).
//!   Empty typed input means `replay_messages` does NOT replace the baseline
//!   via the TurnState (avoiding the replace-vs-accumulate conflict); instead
//!   the MessageAppend lines accumulate. `replay_turn_states` returns the raw
//!   blob verbatim. The real turn-start timestamp rides in `started_at` (F4).
//! - `agent_turn_items` → `TurnItem` lines, appended in `(turn_index, seq)`
//!   order so the derived per-turn `seq` matches.
//!
//! Lines are emitted turn-by-turn (the union of turn indices, ascending) so
//! `read_turn_items` attributes each item to its own turn via the preceding
//! `TurnContext` stamper.
//!
//! # The mixed case + idempotency (G1.2)
//! A thread may carry BOTH legacy SQL rows AND post-migration rollout writes
//! (the common upgrade path: a user continues an existing conversation after
//! the rollout adapter went live). The backfill resolves this by a **full
//! atomic rewrite** via [`RolloutStore::rewrite_session`]:
//! 1. Read all legacy rows from SQL.
//! 2. Read the existing rollout file (flushing first so pending writes are
//!    durable). The lines attributed to a turn STRICTLY GREATER than
//!    `sql_max_turn` (the max legacy turn index) are the **post-migration
//!    tail** — they are NOT covered by the legacy rows and must be preserved.
//!    `SessionMeta` is handled separately.
//! 3. `complete_lines = [SessionMeta] + legacy_lines + post_migration_tail`.
//! 4. `rewrite_session` atomically replaces the file with exactly these lines.
//!
//! Idempotency: `list_thread_ids_for_backfill` excludes completed threads, so a
//! fully-completed thread is never re-run. A crash between the atomic rewrite
//! and `mark_completed` leaves `backfill_status = "in_progress"`; the retry
//! re-derives the SAME `complete_lines` (the SQL legacy is unchanged, and the
//! previously-written legacy rows in the file have `turn <= sql_max_turn` so
//! they are EXCLUDED from the tail — never duplicated) → same rewrite →
//! `completed`. No duplication, no loss.
//!
//! # Concurrency + crash recovery (D2b)
//! A single `backfill_all_threads` run is one worker per process, but a
//! crash-looping process used to re-enter the same thread's atomic rewrite on
//! every boot, and two slab-server processes sharing a DB could race on the
//! same thread. [`backfill_thread`] takes a `lease_owner` and acquires an
//! advisory CAS lease (`try_acquire_backfill_lease`) before rewriting; a thread
//! whose lease is held by a LIVE owner (or not yet expired after a crash) is
//! SKIPPED without being marked failed. The lease is released on success OR
//! failure of the rewrite, so a failed thread is retryable by a later worker
//! without waiting for the TTL (default 15 min).
//!
//! # Known transient gap (G5)
//! For a legacy thread that is actively used BEFORE its backfill completes:
//! writes go to rollout, reads fall back to SQL, so the just-written
//! post-migration turn is invisible until backfill flips the gate. This is a
//! transient gap (backfill runs at startup, desktop history is small); it
//! closes once backfill completes (gate flips → rollout read includes legacy +
//! post-migration). It is NOT the permanent orphan the pre-G1 mixed-case
//! shortcut caused.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use slab_agent_rollout::{
    RolloutFileStore, RolloutItem, RolloutLine, RolloutStore, SessionMeta, TurnContextPayload,
    read_rollout_lines,
};
use slab_types::{ConversationMessage, ConversationMessageContent};
use sqlx::Row;

use crate::error::AppCoreError;
use crate::infra::db::repository::SqlxStore;
use crate::infra::db::repository::rollout_index::RolloutIndex;

/// Backfill lease TTL: a worker that crashes mid-backfill holds the lease for at
/// most this long before another worker may re-acquire it as stale (D2b).
const BACKFILL_LEASE_TTL_SECS: u64 = 900;

/// Batch size for the startup backfill loop: candidates are processed in
/// fixed-size batches with a checkpoint log between batches (D2b). Bounded so a
/// single bulk run emits periodic progress and never holds a huge in-memory
/// window of work.
const BACKFILL_BATCH_SIZE: usize = 200;

/// Outcome of a single-thread backfill (L1). Distinguishes a real completion
/// from a skip so the bulk loop can count + log them separately (a skip is NOT
/// a success — it means another live worker owns the thread or it was already
/// done).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackfillOutcome {
    /// The thread was backfilled: the atomic rewrite + `mark_completed` ran.
    Completed,
    /// No work was done — the thread was already `completed`, or another live
    /// worker holds the backfill lease. The caller logs but does NOT count it
    /// as a success.
    Skipped,
}

/// Build a globally-unique lease owner id for one `backfill_all_threads`
/// invocation. Generated ONCE per bulk run (not per thread) so every thread in
/// the run shares an owner, while a concurrent process (or a later run) gets a
/// distinct owner — letting two workers tell each other's leases apart (D2b).
fn backfill_lease_owner() -> String {
    format!("backfill-pid{}-{}", std::process::id(), uuid::Uuid::new_v4().simple())
}

/// A decoded `agent_turn_states` row held during per-turn grouping.
struct LegacyTurnState {
    status: String,
    input_messages_json: Option<String>,
    tool_specs_json: Option<String>,
    llm_response_json: Option<String>,
    error: Option<String>,
    started_at: String,
    completed_at: Option<String>,
}

/// Backfill a single thread's legacy SQL rows into its rollout JSONL file.
///
/// Handles both the pure-legacy case (no rollout file yet) and the mixed case
/// (legacy SQL rows + post-migration rollout writes coexisting) via a single
/// atomic rewrite. See the module docs for the idempotency + crash-safety
/// argument.
///
/// `lease_owner` identifies the calling worker (one per `backfill_all_threads`
/// run). Before rewriting, this acquires the advisory CAS lease
/// ([`RolloutIndex::try_acquire_backfill_lease`]); a thread whose lease is
/// already held by a LIVE owner is SKIPPED (returned as
/// [`BackfillOutcome::Skipped`], NOT marked failed). The lease is released on
/// success OR failure of the rewrite so a failed thread is retryable without
/// waiting for the TTL. (D2b.)
pub(crate) async fn backfill_thread(
    sqlx: &SqlxStore,
    rollout: &RolloutFileStore,
    thread_id: &str,
    session_id: &str,
    lease_owner: &str,
) -> Result<BackfillOutcome, AppCoreError> {
    // (1) Idempotency: already completed → nothing to do.
    if matches!(
        sqlx.rollout_backfill_status(thread_id).await?,
        Some(status) if status == "completed"
    ) {
        return Ok(BackfillOutcome::Skipped);
    }

    // (2) Acquire the advisory lease BEFORE any rewrite. Ok(true) ⇒ this worker
    // now holds the lease (the CAS stamped status='in_progress' + started_at, so
    // the pre-write `mark_backfill_state("in_progress")` is no longer needed).
    // Ok(false) ⇒ another LIVE owner holds it → SKIP without marking failed
    // (the holder is still working; we must not clobber its state). Err ⇒ a
    // transient SQL error — propagate WITHOUT releasing (we never acquired, so
    // releasing could clear another worker's live lease — M1).
    match sqlx.try_acquire_backfill_lease(thread_id, lease_owner, BACKFILL_LEASE_TTL_SECS).await? {
        true => {}
        false => {
            tracing::info!(
                thread_id = %thread_id,
                lease_owner = %lease_owner,
                "rollout backfill: thread skipped (lease held by another live owner)",
            );
            return Ok(BackfillOutcome::Skipped);
        }
    }

    // (3) Run the rewrite. This worker holds the lease; on EITHER outcome we
    // release it (warn-only, never blocks) so a failed thread is retryable by a
    // later worker without waiting for the TTL. The inner function stamps
    // `mark_backfill_state("completed")` on success and we stamp `"failed"`
    // HERE — only on this worker's own failure path, AFTER it held the lease
    // (so we never overwrite another worker's in_progress with our failed).
    let result = backfill_thread_inner(sqlx, rollout, thread_id, session_id).await;
    if let Err(error) = &result {
        // We held the lease → safe to record our own failure.
        let _ = sqlx.mark_backfill_state(thread_id, "failed", 0, Some(&error.to_string())).await;
    }
    // Release on success AND failure (warn-only). This MUST run for the
    // failure case too: without it a failed thread stays leased until the TTL
    // and is not retryable until then.
    if let Err(error) = sqlx.release_backfill_lease(thread_id).await {
        tracing::warn!(
            thread_id = %thread_id,
            %error,
            "rollout backfill: failed to release backfill lease (will expire at TTL)",
        );
    }
    match result {
        Ok(()) => Ok(BackfillOutcome::Completed),
        Err(error) => Err(error),
    }
}

/// The atomic rewrite itself (the original `backfill_thread` body), MINUS the
/// pre-write `mark_backfill_state("in_progress")`: the lease CAS already
/// stamped `status='in_progress' + started_at`, so a second write would
/// needlessly overwrite it. On success this stamps
/// `mark_backfill_state("completed")`; on failure it returns `Err` and leaves
/// the terminal `"failed"` stamp to the caller (which knows this worker holds
/// the lease). Holds the lease on entry; the caller releases it.
async fn backfill_thread_inner(
    sqlx: &SqlxStore,
    rollout: &RolloutFileStore,
    thread_id: &str,
    session_id: &str,
) -> Result<(), AppCoreError> {
    // (1) Read the legacy thread metadata. Used to build the SessionMeta header
    // when the rollout file does not yet carry one, and to no-op when the
    // thread vanished (deleted concurrently).
    let thread_row = sqlx::query(
        "SELECT id, session_id, parent_id, created_at, config_json, role_name \
         FROM agent_threads WHERE id = ?1",
    )
    .bind(thread_id)
    .fetch_optional(&sqlx.pool)
    .await?;
    let Some(thread_row) = thread_row else {
        // Thread vanished (deleted concurrently) — nothing to backfill.
        return Ok(());
    };
    let parent_id: Option<String> = thread_row.try_get("parent_id")?;
    let created_at: String = thread_row.try_get("created_at")?;
    let config_json: String = thread_row.try_get("config_json")?;
    let role_name: Option<String> = thread_row.try_get("role_name")?;
    let config_value = serde_json::from_str(&config_json).unwrap_or_else(|_| serde_json::json!({}));

    // (3) Read the three legacy tables (read-only source) ordered as the SQL
    // store returns them, so the rollout replay matches the SQL read order.
    let message_rows = sqlx::query(
        "SELECT id, turn_index, role, content, created_at \
         FROM agent_thread_messages WHERE thread_id = ?1 \
         ORDER BY turn_index ASC, created_at ASC, id ASC",
    )
    .bind(thread_id)
    .fetch_all(&sqlx.pool)
    .await?;

    let state_rows = sqlx::query(
        "SELECT turn_index, status, input_messages_json, tool_specs_json, \
                llm_response_json, error, started_at, completed_at \
         FROM agent_turn_states WHERE thread_id = ?1 ORDER BY turn_index ASC",
    )
    .bind(thread_id)
    .fetch_all(&sqlx.pool)
    .await?;

    let item_rows = sqlx::query(
        "SELECT id, turn_index, seq, item_json, created_at \
         FROM agent_turn_items WHERE thread_id = ?1 \
         ORDER BY turn_index ASC, seq ASC",
    )
    .bind(thread_id)
    .fetch_all(&sqlx.pool)
    .await?;

    // Group by turn_index (i64) so we can emit each turn's messages + state +
    // items together — required for read_turn_items to attribute items to the
    // right turn via the preceding TurnContext stamper.
    let mut messages_by_turn: BTreeMap<i64, Vec<(String, String, String, String)>> =
        BTreeMap::new();
    for row in &message_rows {
        let id: String = row.try_get("id")?;
        let turn_index: i64 = row.try_get("turn_index")?;
        let role: String = row.try_get("role")?;
        let content: String = row.try_get("content")?;
        let created_at: String = row.try_get("created_at")?;
        messages_by_turn.entry(turn_index).or_default().push((id, role, content, created_at));
    }

    let mut states_by_turn: BTreeMap<i64, LegacyTurnState> = BTreeMap::new();
    for row in &state_rows {
        let turn_index: i64 = row.try_get("turn_index")?;
        states_by_turn.insert(
            turn_index,
            LegacyTurnState {
                status: row.try_get("status")?,
                input_messages_json: row.try_get("input_messages_json")?,
                tool_specs_json: row.try_get("tool_specs_json")?,
                llm_response_json: row.try_get("llm_response_json")?,
                error: row.try_get("error")?,
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
            },
        );
    }

    let mut items_by_turn: BTreeMap<i64, Vec<(String, i64, String, String)>> = BTreeMap::new();
    for row in &item_rows {
        let id: String = row.try_get("id")?;
        let turn_index: i64 = row.try_get("turn_index")?;
        let seq: i64 = row.try_get("seq")?;
        let item_json: String = row.try_get("item_json")?;
        let created_at: String = row.try_get("created_at")?;
        items_by_turn.entry(turn_index).or_default().push((id, seq, item_json, created_at));
    }

    // Union of legacy turn indices, ascending.
    let mut all_legacy_turns: BTreeSet<i64> = messages_by_turn.keys().copied().collect();
    all_legacy_turns.extend(states_by_turn.keys().copied());
    all_legacy_turns.extend(items_by_turn.keys().copied());
    let sql_max_turn: i64 = all_legacy_turns.iter().rev().copied().next().unwrap_or(-1);
    let has_legacy = !all_legacy_turns.is_empty();

    // (4) Read existing rollout lines (flush first so pending writes are
    // durable). For a pure-legacy thread the file does not exist yet → empty.
    // Uses `resolve_path` (the read reverse-lookup chain: session_index →
    // date-partitioned scan → legacy flat fallback), NOT the write-only
    // `path_for_new`. (D1.)
    let _ = rollout.flush(thread_id).await;
    let existing_lines: Vec<RolloutLine> = read_rollout_lines(&rollout.resolve_path(thread_id));

    // (5) SessionMeta line: prefer the existing file's header (it carries the
    // real config / session_id stamped by the adapter's upsert_thread); else
    // build one from the agent_threads row.
    let session_meta_line = existing_lines
        .iter()
        .find_map(|line| match &line.item {
            RolloutItem::SessionMeta(_) => Some(line.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            RolloutLine::with_timestamp(
                created_at.clone(),
                RolloutItem::SessionMeta(SessionMeta {
                    thread_id: thread_id.to_owned(),
                    session_id: session_id.to_owned(),
                    parent_id,
                    started_at: created_at,
                    config_json: config_value,
                    rollout_version: SessionMeta::CURRENT_VERSION,
                    role_name,
                    trace_path: None,
                }),
            )
        });

    // (6) Post-migration tail: existing lines (EXCLUDING SessionMeta) whose
    // attributed turn is strictly greater than sql_max_turn. When there is no
    // legacy data at all, keep every non-SessionMeta line (the whole file is
    // post-migration). This is the idempotency key: on retry after a prior
    // rewrite, the legacy rows in the file have turn <= sql_max_turn and are
    // EXCLUDED, so they are never duplicated.
    let post_migration_tail =
        extract_post_migration_tail(&existing_lines, sql_max_turn, has_legacy);

    // (7) Build the legacy rollout lines turn-by-turn (union ascending). Within
    // a turn: messages, then the turn state, then items (seq order). Each line
    // preserves its original SQL timestamp.
    let mut legacy_lines: Vec<RolloutLine> = Vec::new();
    for turn in &all_legacy_turns {
        let turn_index = u32::try_from(*turn).map_err(|error| {
            AppCoreError::Internal(format!(
                "invalid turn_index {turn} for thread {thread_id}: {error}"
            ))
        })?;

        // (a) Messages → MessageAppend (F3 carries id + created_at).
        if let Some(messages) = messages_by_turn.get(turn) {
            for (id, role, content, created_at) in messages {
                let message = decode_message(id, role, content);
                legacy_lines.push(RolloutLine::with_timestamp(
                    created_at.clone(),
                    RolloutItem::TurnContext(TurnContextPayload::MessageAppend {
                        turn_index,
                        message,
                        id: Some(id.clone()),
                        created_at: Some(created_at.clone()),
                    }),
                ));
            }
        }

        // (b) Turn state → TurnState with EMPTY typed input (raw blob in F6).
        if let Some(state) = states_by_turn.get(turn) {
            legacy_lines.push(RolloutLine::with_timestamp(
                state.started_at.clone(),
                RolloutItem::TurnContext(TurnContextPayload::TurnState {
                    turn_index,
                    status: state.status.clone(),
                    input_messages: Vec::new(),
                    tool_specs_json: state.tool_specs_json.clone(),
                    llm_response_json: state.llm_response_json.clone(),
                    error: state.error.clone(),
                    completed_at: state.completed_at.clone(),
                    started_at: Some(state.started_at.clone()),
                    input_messages_raw: state.input_messages_json.clone(),
                }),
            ));
        }

        // (c) Turn items → TurnItem, in seq order so the derived per-turn seq
        // matches.
        if let Some(items) = items_by_turn.get(turn) {
            // The query already ordered by seq; preserve that order.
            for (_id, _seq, item_json, created_at) in items {
                let item: slab_agent::protocol::TurnItem = serde_json::from_str(item_json)
                    .map_err(|error| {
                        AppCoreError::Internal(format!(
                            "failed to decode TurnItem for thread {thread_id}: {error}"
                        ))
                    })?;
                legacy_lines.push(RolloutLine::with_timestamp(
                    created_at.clone(),
                    RolloutItem::TurnItem(item),
                ));
            }
        }
    }

    // (8) complete_lines = [SessionMeta] + legacy + post-migration tail. Pull
    // the SessionMeta out of the header line BEFORE moving it into the vec — it
    // is also needed below to seed the recorder at the canonical path (D1).
    let session_meta = match &session_meta_line.item {
        RolloutItem::SessionMeta(meta) => meta.clone(),
        // session_meta_line is always built as a SessionMeta above.
        _ => unreachable!("session_meta_line is always a SessionMeta line"),
    };
    let mut complete_lines: Vec<RolloutLine> =
        Vec::with_capacity(1 + legacy_lines.len() + post_migration_tail.len());
    complete_lines.push(session_meta_line);
    complete_lines.extend(legacy_lines);
    complete_lines.extend(post_migration_tail);

    // (9) Ensure a recorder exists at the CANONICAL path before the rewrite
    // (D1). `create_session` is idempotent: for a PURE-LEGACY thread (no
    // existing rollout file) it spawns a `Create` recorder at the
    // date-partitioned `path_for_new` path derived from `meta.started_at` and
    // appends a `session_index.jsonl` entry — so the rewritten file lands in
    // the NEW layout (not the legacy flat fallback). For a MIXED thread it
    // spawns a `Resume` recorder on the EXISTING file's path (found via
    // `lookup_path`), so the rewrite preserves the existing path. The rewrite
    // itself (next line) routes through that recorder when present.
    rollout.create_session(session_meta);

    // (10) Atomic rewrite — replaces the file with exactly these lines. Handles
    // both the pure-legacy case (creates the file at path_for_new via the
    // recorder) and the mixed case (merges legacy prefix + post-migration tail
    // on the existing path). Idempotent + crash-safe (see module docs).
    rollout.rewrite_session(thread_id, complete_lines.clone()).await.map_err(map_rollout_error)?;

    // (11) Compute the real last_turn_index / last_item_id / line_count from the
    // complete line set, then flip the index to completed. last_turn_index is
    // the max turn across legacy + post-migration; last_item_id is the last
    // TurnItem's id. The file path is resolved via the read reverse-lookup chain
    // (now finds the just-written file).
    let last_turn_index =
        complete_lines.iter().filter_map(|line| line.item.turn_index()).max().unwrap_or(0);
    let last_item_id = complete_lines.iter().rev().find_map(|line| match &line.item {
        RolloutItem::TurnItem(ti) => Some(ti.id().to_owned()),
        _ => None,
    });
    let line_count = u32::try_from(complete_lines.len()).unwrap_or(0);
    let file_path = rollout.resolve_path(thread_id).to_string_lossy().into_owned();

    mark_completed(
        sqlx,
        thread_id,
        session_id,
        &file_path,
        last_turn_index,
        last_item_id.as_deref(),
        line_count,
    )
    .await?;
    let _ = sqlx.mark_backfill_state(thread_id, "completed", line_count, None).await;
    Ok(())
}

/// Select the post-migration tail from the existing rollout lines.
///
/// Lines attributed to a turn STRICTLY greater than `sql_max_turn` are
/// post-migration (the legacy SQL rows cover turns `<= sql_max_turn`). When
/// `has_legacy` is false (no legacy data at all), every non-SessionMeta line is
/// kept — the whole file is post-migration and a `>` test against the sentinel
/// `-1` would still drop legitimate turn-0 content.
///
/// Turn attribution mirrors `read_turn_items`: a `TurnContext` /
/// `Compacted` line stamps the running turn; `TurnItem` / `EventMsg` lines
/// inherit the most recently stamped turn. `SessionMeta` is always excluded
/// (handled separately by the caller).
fn extract_post_migration_tail(
    lines: &[RolloutLine],
    sql_max_turn: i64,
    has_legacy: bool,
) -> Vec<RolloutLine> {
    let mut tail = Vec::new();
    let mut running_turn: i64 = 0;
    for line in lines {
        let attributed_turn = match &line.item {
            RolloutItem::SessionMeta(_) => continue,
            RolloutItem::TurnContext(tc) => {
                running_turn = i64::from(tc.turn_index());
                running_turn
            }
            RolloutItem::Compacted(payload) => {
                running_turn = i64::from(payload.turn_index);
                running_turn
            }
            RolloutItem::TurnItem(_) | RolloutItem::EventMsg(_) => running_turn,
        };
        let keep = if has_legacy { attributed_turn > sql_max_turn } else { true };
        if keep {
            tail.push(line.clone());
        }
    }
    tail
}

/// Backfill every thread not yet `completed`. Spawned fire-and-forget at
/// startup; returns `(succeeded, skipped, failed)` for diagnostics logging
/// (L1: `skipped` is reported separately — a skip is NOT a success).
///
/// Candidates are processed in fixed-size batches ([`BACKFILL_BATCH_SIZE`])
/// with a checkpoint log between batches so a long run emits periodic progress
/// (D2b). A single `lease_owner` is generated for the whole run and passed to
/// each [`backfill_thread`], so every thread in this run shares an owner while
/// a concurrent process gets a distinct one.
pub async fn backfill_all_threads(
    sqlx: Arc<SqlxStore>,
    rollout: Arc<RolloutFileStore>,
) -> (usize, usize, usize) {
    let candidates = match sqlx.list_thread_ids_for_backfill().await {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::warn!(
                %error,
                "rollout backfill: failed to list candidate threads; legacy threads stay on SQL",
            );
            return (0, 0, 0);
        }
    };

    // One owner per bulk run (D2b): every thread shares it, a concurrent
    // process gets a different one.
    let owner = backfill_lease_owner();
    let total = candidates.len();
    let mut succeeded = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    for (batch_idx, chunk) in candidates.chunks(BACKFILL_BATCH_SIZE).enumerate() {
        for (thread_id, session_id) in chunk {
            match backfill_thread(&sqlx, &rollout, thread_id, session_id, &owner).await {
                Ok(BackfillOutcome::Completed) => succeeded += 1,
                Ok(BackfillOutcome::Skipped) => {
                    skipped += 1;
                    tracing::info!(
                        thread_id = %thread_id,
                        "rollout backfill: thread skipped (already completed or lease held)",
                    );
                }
                Err(error) => {
                    // M1: do NOT release_backfill_lease here and do NOT
                    // mark_backfill_state("failed") here. `backfill_thread`
                    // already released this worker's OWN lease and stamped its
                    // OWN "failed" on its failure path (only when it held the
                    // lease). If we also released/marked here we could clobber
                    // ANOTHER worker's live lease: when `try_acquire_backfill_lease`
                    // itself returned Err (transient SQL error, we never
                    // acquired), an unconditional release would clear whoever
                    // currently holds the lease, and a `mark_failed` would
                    // overwrite their in_progress — letting a third worker
                    // re-enter a thread still being backfilled (the exact
                    // double-rewrite the lease prevents).
                    failed += 1;
                    tracing::warn!(
                        thread_id = %thread_id,
                        %error,
                        "rollout backfill: thread failed; it stays on the SQL read path",
                    );
                }
            }
        }
        tracing::info!(
            batch = batch_idx,
            batch_len = chunk.len(),
            total,
            succeeded,
            skipped,
            failed,
            worker = %owner,
            "rollout backfill: batch checkpoint",
        );
    }
    (succeeded, skipped, failed)
}

/// Decode a stored `agent_thread_messages.content` blob into a
/// [`ConversationMessage`], mirroring the SQL store's fallback: on parse
/// failure, wrap the raw text with the stored `role` so a malformed row is
/// recoverable instead of dropped.
fn decode_message(id: &str, role: &str, content: &str) -> ConversationMessage {
    serde_json::from_str::<ConversationMessage>(content).unwrap_or_else(|error| {
        tracing::warn!(
            message_id = %id,
            role = %role,
            error = %error,
            "backfill: failed to decode stored message content; preserving raw text",
        );
        ConversationMessage {
            role: role.to_owned(),
            content: ConversationMessageContent::Text(content.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    })
}

async fn mark_completed(
    sqlx: &SqlxStore,
    thread_id: &str,
    session_id: &str,
    file_path: &str,
    last_turn_index: u32,
    last_item_id: Option<&str>,
    line_count: u32,
) -> Result<(), AppCoreError> {
    sqlx.mark_rollout_session(
        thread_id,
        session_id,
        file_path,
        last_turn_index,
        last_item_id,
        line_count,
        "completed",
    )
    .await?;
    Ok(())
}

fn map_rollout_error(error: slab_agent_rollout::RolloutError) -> AppCoreError {
    AppCoreError::Internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::agent::rollout_store::RolloutBackedAgentStore;
    use crate::infra::db::repository::SqlxStore;
    use crate::infra::db::repository::rollout_index::RolloutIndex;
    use crate::test_support::migrated_test_store;
    use slab_agent::port::{AgentStorePort, ThreadMessageRecord, ThreadSnapshot, ThreadStatus};
    use slab_agent::protocol::TurnItem;
    use slab_types::ConversationMessageContent;

    /// A fresh migrated in-memory store for backfill tests.
    async fn store() -> SqlxStore {
        migrated_test_store().await
    }

    fn now() -> &'static str {
        "2026-01-01T00:00:00Z"
    }

    async fn seed_session_thread(sqlx: &SqlxStore, thread_id: &str, session_id: &str) {
        sqlx::query(
            "INSERT INTO chat_sessions (id, created_at, updated_at) \
             VALUES (?1, ?2, ?2)",
        )
        .bind(session_id)
        .bind(now())
        .execute(&sqlx.pool)
        .await
        .expect("seed session");
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, parent_id, depth, status, \
             config_json, created_at, updated_at) \
             VALUES (?1, ?2, NULL, 0, 'completed', '{\"model\":\"m\"}', ?3, ?3)",
        )
        .bind(thread_id)
        .bind(session_id)
        .bind(now())
        .execute(&sqlx.pool)
        .await
        .expect("seed agent_thread");
    }

    async fn seed_message(
        sqlx: &SqlxStore,
        id: &str,
        thread_id: &str,
        turn_index: i64,
        message: &ConversationMessage,
        created_at: &str,
    ) {
        let content = serde_json::to_string(message).expect("serialize message");
        sqlx::query(
            "INSERT INTO agent_thread_messages (id, thread_id, turn_index, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(thread_id)
        .bind(turn_index)
        .bind(&message.role)
        .bind(content)
        .bind(created_at)
        .execute(&sqlx.pool)
        .await
        .expect("seed message");
    }

    async fn seed_state(
        sqlx: &SqlxStore,
        thread_id: &str,
        turn_index: i64,
        status: &str,
        input_messages_json: Option<&str>,
        started_at: &str,
        completed_at: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO agent_turn_states \
             (thread_id, turn_index, status, input_messages_json, tool_specs_json, \
              llm_response_json, error, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
        )
        .bind(thread_id)
        .bind(turn_index)
        .bind(status)
        .bind(input_messages_json)
        .bind("[\"toolA\"]")
        .bind("{\"id\":\"resp\"}")
        .bind(started_at)
        .bind(completed_at)
        .execute(&sqlx.pool)
        .await
        .expect("seed turn state");
    }

    async fn seed_item(
        sqlx: &SqlxStore,
        id: &str,
        thread_id: &str,
        turn_index: i64,
        seq: i64,
        item: &TurnItem,
    ) {
        let item_json = serde_json::to_string(item).expect("serialize turn item");
        sqlx::query(
            "INSERT INTO agent_turn_items (id, thread_id, turn_index, seq, item_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(thread_id)
        .bind(turn_index)
        .bind(seq)
        .bind(item_json)
        .bind(now())
        .execute(&sqlx.pool)
        .await
        .expect("seed turn item");
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

    fn assistant_msg(text: &str) -> ConversationMessage {
        ConversationMessage {
            role: "assistant".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
    }

    fn tool_msg(text: &str) -> ConversationMessage {
        ConversationMessage {
            role: "tool".to_owned(),
            content: ConversationMessageContent::Text(text.to_owned()),
            name: None,
            tool_call_id: Some("call-1".to_owned()),
            tool_calls: vec![],
        }
    }

    fn rollout_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("rollout temp dir")
    }

    // The fidelity guarantee: after backfill, the rollout adapter reconstructs
    // the SAME data the SQL store returns (messages incl. original id /
    // created_at via F3; turn-state fields incl. input_messages_json via F6 and
    // started_at via F4; turn-item ids/texts). G4(a): after backfill, the legacy
    // SQL rows are DELETED so a fallback-to-SQL regression would return empty
    // and FAIL — the rollout-served data is proven to equal the original, not
    // the still-populated SQL.
    #[tokio::test]
    async fn backfill_thread_migrates_legacy_thread_with_fidelity() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        seed_session_thread(&sqlx, "thread-legacy", "session-legacy").await;
        // Two turns of messages (incl. a tool-role message + a multi-message turn).
        seed_message(&sqlx, "m0", "thread-legacy", 0, &user_msg("hello"), "2026-01-01T00:00:01Z")
            .await;
        seed_message(
            &sqlx,
            "m1",
            "thread-legacy",
            1,
            &assistant_msg("hi back"),
            "2026-01-01T00:00:10Z",
        )
        .await;
        // A tool-role message in turn 1 (tool_call_id set).
        seed_message(
            &sqlx,
            "m-tool",
            "thread-legacy",
            1,
            &tool_msg("tool-result"),
            "2026-01-01T00:00:11Z",
        )
        .await;
        // Turn states (input blob + started_at must survive via F6/F4).
        seed_state(
            &sqlx,
            "thread-legacy",
            0,
            "completed",
            Some(r#"[{"role":"user","content":"hello"}]"#),
            "2026-01-01T00:00:00Z",
            Some("2026-01-01T00:00:05Z"),
        )
        .await;
        seed_state(
            &sqlx,
            "thread-legacy",
            1,
            "completed",
            Some(r#"[{"role":"user","content":"hello"},{"role":"assistant","content":"hi back"}]"#),
            "2026-01-01T00:00:09Z",
            None,
        )
        .await;
        // Turn items (contiguous seq per turn).
        seed_item(
            &sqlx,
            "i0",
            "thread-legacy",
            0,
            0,
            &TurnItem::AgentMessage { id: "i0".to_owned(), text: "ack0".to_owned() },
        )
        .await;
        seed_item(
            &sqlx,
            "i1",
            "thread-legacy",
            1,
            0,
            &TurnItem::AgentMessage { id: "i1".to_owned(), text: "ack1".to_owned() },
        )
        .await;

        // SQL reads (the source of truth before backfill).
        let sql_messages = sqlx.list_thread_messages("thread-legacy").await.expect("sql messages");
        let sql_items = sqlx.list_turn_items("thread-legacy").await.expect("sql items");
        let sql_states = sqlx.list_turn_states("thread-legacy").await.expect("sql states");

        // Run the backfill.
        backfill_thread(&sqlx, &rollout, "thread-legacy", "session-legacy", "test-owner")
            .await
            .expect("backfill");

        // The index is now completed.
        assert_eq!(
            sqlx.rollout_backfill_status("thread-legacy").await.unwrap().as_deref(),
            Some("completed"),
        );

        // G4(b): line_count was stamped non-zero.
        let line_count: i64 = sqlx::query_scalar(
            "SELECT line_count FROM rollout_session_index WHERE thread_id = 'thread-legacy'",
        )
        .fetch_one(&sqlx.pool)
        .await
        .expect("line_count");
        assert!(line_count > 0, "line_count stamped non-zero after backfill");

        // G4(a): DELETE the legacy SQL rows so a fallback-to-SQL regression
        // returns EMPTY and FAILS. This proves the rollout read is authoritative.
        sqlx::query("DELETE FROM agent_thread_messages WHERE thread_id = 'thread-legacy'")
            .execute(&sqlx.pool)
            .await
            .expect("delete legacy messages");
        sqlx::query("DELETE FROM agent_turn_states WHERE thread_id = 'thread-legacy'")
            .execute(&sqlx.pool)
            .await
            .expect("delete legacy states");
        sqlx::query("DELETE FROM agent_turn_items WHERE thread_id = 'thread-legacy'")
            .execute(&sqlx.pool)
            .await
            .expect("delete legacy items");

        // Build an adapter over the SAME sqlx store (as both delegate + index)
        // and read through it — reads must now come from rollout and match the
        // ORIGINAL SQL snapshot.
        let sqlx_arc = Arc::new(sqlx);
        let adapter = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx_arc) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx_arc) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        );
        let rl_messages =
            adapter.list_thread_messages("thread-legacy").await.expect("rollout messages");
        let rl_items = adapter.list_turn_items("thread-legacy").await.expect("rollout items");
        let rl_states = adapter.list_turn_states("thread-legacy").await.expect("rollout states");

        // Messages: same count, ids, roles, contents, turn_index, created_at (F3).
        assert_eq!(rl_messages.len(), sql_messages.len(), "message count matches");
        for (rl, sql) in rl_messages.iter().zip(sql_messages.iter()) {
            assert_eq!(rl.id, sql.id, "message id (F3)");
            assert_eq!(rl.turn_index, sql.turn_index);
            assert_eq!(rl.created_at, sql.created_at, "message created_at (F3)");
            assert_eq!(rl.message.role, sql.message.role);
            assert_eq!(rl.message.content.rendered_text(), sql.message.content.rendered_text());
        }
        // The tool-role message survived (role + tool_call_id).
        assert!(
            rl_messages.iter().any(|m| m.message.role == "tool"),
            "tool-role message survived the migration"
        );

        // Turn items: same ids, turn_index, derived seq (contiguous), decoded text.
        assert_eq!(rl_items.len(), sql_items.len(), "item count matches");
        for (rl, sql) in rl_items.iter().zip(sql_items.iter()) {
            assert_eq!(rl.id, sql.id);
            assert_eq!(rl.turn_index, sql.turn_index);
            assert_eq!(rl.seq, sql.seq, "derived seq matches contiguous source seq");
            let rl_ti: TurnItem = serde_json::from_str(&rl.item_json).expect("decode rl item");
            let sql_ti: TurnItem = serde_json::from_str(&sql.item_json).expect("decode sql item");
            assert_eq!(rl_ti, sql_ti, "turn item payload matches");
        }

        // Turn states: turn_index, status, input_messages_json (F6), started_at (F4),
        // tool_specs_json, llm_response_json, completed_at.
        assert_eq!(rl_states.len(), sql_states.len(), "state count matches");
        for (rl, sql) in rl_states.iter().zip(sql_states.iter()) {
            assert_eq!(rl.turn_index, sql.turn_index);
            assert_eq!(rl.status, sql.status);
            assert_eq!(
                rl.input_messages_json, sql.input_messages_json,
                "input_messages_json preserved via F6 raw blob",
            );
            assert_eq!(rl.started_at, sql.started_at, "started_at recovered via F4");
            assert_eq!(rl.tool_specs_json, sql.tool_specs_json);
            assert_eq!(rl.llm_response_json, sql.llm_response_json);
            assert_eq!(rl.completed_at, sql.completed_at);
        }
    }

    // The read gate: a legacy thread (in SQL, no rollout file, no index row)
    // reads from SQL BEFORE backfill; AFTER backfill completes, reads come from
    // rollout and match. G4(a): the legacy SQL row is DELETED after backfill so
    // a fallback-to-SQL regression returns empty and FAILS.
    #[tokio::test]
    async fn read_gate_switches_sql_to_rollout_after_backfill() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        seed_session_thread(&sqlx, "thread-gate", "session-gate").await;
        seed_message(
            &sqlx,
            "gm0",
            "thread-gate",
            0,
            &user_msg("legacy-payload"),
            "2026-01-01T00:00:00Z",
        )
        .await;

        let sqlx_arc = Arc::new(sqlx);
        let adapter = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx_arc) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx_arc) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        );

        // BEFORE backfill: no index row → SQL fallback → the legacy message is
        // visible (NOT orphaned to an empty rollout read).
        assert_eq!(
            sqlx_arc.rollout_backfill_status("thread-gate").await.unwrap(),
            None,
            "no index row yet",
        );
        let before = adapter.list_thread_messages("thread-gate").await.expect("read before");
        assert_eq!(before.len(), 1, "legacy thread reads from SQL");
        assert_eq!(before[0].id, "gm0");

        // Run the backfill (flips the gate to completed).
        backfill_thread(&sqlx_arc, &rollout, "thread-gate", "session-gate", "test-owner")
            .await
            .expect("backfill");
        assert_eq!(
            sqlx_arc.rollout_backfill_status("thread-gate").await.unwrap().as_deref(),
            Some("completed"),
        );

        // G4(a): DELETE the legacy SQL row so a SQL-fallback regression returns
        // EMPTY. The rollout read must still surface the migrated message.
        sqlx::query("DELETE FROM agent_thread_messages WHERE thread_id = 'thread-gate'")
            .execute(&sqlx_arc.pool)
            .await
            .expect("delete legacy row");

        // AFTER backfill: rollout-first, and the legacy message survives.
        let after = adapter.list_thread_messages("thread-gate").await.expect("read after");
        assert_eq!(after.len(), 1, "rollout-first after backfill (not empty SQL fallback)");
        assert_eq!(after[0].id, "gm0", "legacy message survived the migration");
        assert_eq!(after[0].message.content.rendered_text(), "legacy-payload");
    }

    // A brand-new thread is rollout-native immediately: upsert_thread marks it
    // completed (no legacy data), so reads go rollout-first with no backfill.
    #[tokio::test]
    async fn new_thread_is_rollout_native_without_backfill() {
        let sqlx = store().await;
        sqlx::query("INSERT INTO chat_sessions (id, created_at, updated_at) VALUES ('s1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')")
            .execute(&sqlx.pool)
            .await
            .expect("session");
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        let sqlx_arc = Arc::new(sqlx);
        let adapter = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx_arc) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx_arc) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        );

        adapter
            .upsert_thread(&ThreadSnapshot {
                id: "thread-new".to_owned(),
                session_id: "s1".to_owned(),
                parent_id: None,
                depth: 0,
                status: ThreadStatus::Running,
                role_name: None,
                config_json: "{}".to_owned(),
                completion_text: None,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                archived_at: None,
            })
            .await
            .expect("upsert new thread");

        // Immediately completed (no backfill needed) — born on rollout.
        assert_eq!(
            sqlx_arc.rollout_backfill_status("thread-new").await.unwrap().as_deref(),
            Some("completed"),
        );

        // A write + read round-trips through rollout (no SQL data written).
        adapter
            .insert_thread_message(&ThreadMessageRecord {
                id: "nm0".to_owned(),
                thread_id: "thread-new".to_owned(),
                turn_index: 0,
                message: user_msg("born-on-rollout"),
                created_at: "2026-01-01T00:00:01Z".to_owned(),
            })
            .await
            .expect("insert");
        let messages = adapter.list_thread_messages("thread-new").await.expect("read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, "nm0");
    }

    // Idempotency: backfill_thread twice does not duplicate rollout lines and
    // the index stays completed.
    #[tokio::test]
    async fn backfill_thread_is_idempotent() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        seed_session_thread(&sqlx, "thread-idem", "session-idem").await;
        seed_message(&sqlx, "im0", "thread-idem", 0, &user_msg("once"), "2026-01-01T00:00:00Z")
            .await;

        backfill_thread(&sqlx, &rollout, "thread-idem", "session-idem", "test-owner")
            .await
            .expect("first backfill");
        let lines_after_first = read_rollout_lines(&rollout.resolve_path("thread-idem")).len();

        // Second run must be a no-op (index already completed).
        backfill_thread(&sqlx, &rollout, "thread-idem", "session-idem", "test-owner")
            .await
            .expect("second backfill");
        let lines_after_second = read_rollout_lines(&rollout.resolve_path("thread-idem")).len();

        assert_eq!(lines_after_first, lines_after_second, "no duplicate lines on re-run");
        assert_eq!(
            sqlx.rollout_backfill_status("thread-idem").await.unwrap().as_deref(),
            Some("completed"),
        );
    }

    // backfill_all_threads picks up un-backfilled threads, skips completed ones,
    // and returns (succeeded, failed).
    #[tokio::test]
    async fn backfill_all_threads_processes_pending_skips_completed() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        seed_session_thread(&sqlx, "thread-a", "session-a").await;
        seed_message(&sqlx, "am0", "thread-a", 0, &user_msg("a"), "2026-01-01T00:00:00Z").await;
        seed_session_thread(&sqlx, "thread-b", "session-b").await;
        seed_message(&sqlx, "bm0", "thread-b", 0, &user_msg("b"), "2026-01-01T00:00:00Z").await;

        // Pre-mark thread-b completed — it must be skipped by the bulk run.
        sqlx.mark_rollout_session("thread-b", "session-b", "x", 0, None, 0, "completed")
            .await
            .expect("pre-mark b");

        let sqlx_arc = Arc::new(sqlx);
        let (succeeded, skipped, failed) =
            backfill_all_threads(Arc::clone(&sqlx_arc), Arc::clone(&rollout)).await;
        assert_eq!(succeeded, 1, "only thread-a needed backfilling");
        assert_eq!(skipped, 0, "thread-b was excluded by the candidate query, not skipped");
        assert_eq!(failed, 0);
        assert_eq!(
            sqlx_arc.rollout_backfill_status("thread-a").await.unwrap().as_deref(),
            Some("completed"),
        );
        // thread-b's rollout file was NOT created (it was skipped).
        assert!(
            !rollout.file_exists("thread-b").await,
            "completed thread skipped, no file written"
        );
    }

    // G1.2 (the core fix): the MIXED case — a thread with legacy SQL rows AND a
    // post-migration rollout write — must MERGE both into the rollout file. The
    // pre-fix code orphaned the legacy prefix the moment the rollout file
    // materialized. This test seeds a legacy thread, performs ONE post-migration
    // write through the adapter (turn_index > legacy turns) so the rollout file
    // materializes, runs backfill, then asserts BOTH survive. It also asserts
    // idempotency: re-running after a simulated retry leaves the rollout read
    // unchanged (no duplication).
    #[tokio::test]
    async fn backfill_merges_legacy_and_post_migration_in_mixed_case() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        seed_session_thread(&sqlx, "thread-mixed", "session-mixed").await;
        // Legacy turn 0: a user message.
        seed_message(
            &sqlx,
            "lm0",
            "thread-mixed",
            0,
            &user_msg("legacy-turn-0"),
            "2026-01-01T00:00:00Z",
        )
        .await;

        let sqlx_arc = Arc::new(sqlx);
        // Build the adapter. upsert_thread stamps the real SessionMeta (the
        // thread has legacy data, so it is NOT marked completed here). Then
        // perform ONE post-migration write through it with a NEW turn_index (5)
        // strictly greater than the legacy turn (0). This materializes the
        // rollout file with post-migration content.
        let adapter = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx_arc) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx_arc) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        );
        adapter
            .upsert_thread(&ThreadSnapshot {
                id: "thread-mixed".to_owned(),
                session_id: "session-mixed".to_owned(),
                parent_id: None,
                depth: 0,
                status: ThreadStatus::Running,
                role_name: None,
                config_json: "{}".to_owned(),
                completion_text: None,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                archived_at: None,
            })
            .await
            .expect("upsert (does NOT mark completed — legacy data present)");
        adapter
            .insert_thread_message(&ThreadMessageRecord {
                id: "pm5".to_owned(),
                thread_id: "thread-mixed".to_owned(),
                turn_index: 5,
                message: user_msg("post-migration-turn-5"),
                created_at: "2026-02-02T00:00:00Z".to_owned(),
            })
            .await
            .expect("post-migration write");
        // The rollout file now materialized (with the post-migration write +
        // adapter-stamped SessionMeta, but NO legacy prefix).

        // Run the backfill — must MERGE the legacy turn-0 message with the
        // post-migration turn-5 message.
        backfill_thread(&sqlx_arc, &rollout, "thread-mixed", "session-mixed", "test-owner")
            .await
            .expect("backfill mixed");

        assert_eq!(
            sqlx_arc.rollout_backfill_status("thread-mixed").await.unwrap().as_deref(),
            Some("completed"),
        );

        // Read through the adapter: BOTH the legacy message AND the
        // post-migration message must be present (pre-fix code orphaned the
        // legacy prefix).
        let messages = adapter.list_thread_messages("thread-mixed").await.expect("read");
        assert_eq!(messages.len(), 2, "legacy + post-migration both present");
        let ids: Vec<&str> = messages.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"lm0"), "legacy message survived (not orphaned)");
        assert!(ids.contains(&"pm5"), "post-migration message survived");
        // Replay order: legacy (turn 0) before post-migration (turn 5).
        assert_eq!(messages[0].id, "lm0");
        assert_eq!(messages[1].id, "pm5");

        // Idempotency: simulate a crash-retry by resetting backfill_status to
        // in_progress and re-running. The rollout read must be UNCHANGED (no
        // duplication of the legacy prefix).
        sqlx::query("UPDATE rollout_session_index SET backfill_status = 'in_progress' WHERE thread_id = 'thread-mixed'")
            .execute(&sqlx_arc.pool)
            .await
            .expect("reset status");
        backfill_thread(&sqlx_arc, &rollout, "thread-mixed", "session-mixed", "test-owner")
            .await
            .expect("retry backfill");
        let messages_after_retry =
            adapter.list_thread_messages("thread-mixed").await.expect("read after retry");
        assert_eq!(
            messages_after_retry.len(),
            2,
            "no duplication on retry — legacy prefix not re-written"
        );
    }

    // G4(c): an empty legacy thread (no rows in any of the three tables, but a
    // thread row exists) backfills to an empty rollout (just SessionMeta) and
    // the index is marked completed.
    #[tokio::test]
    async fn backfill_empty_legacy_thread_produces_empty_rollout() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        // Thread metadata only — no messages, states, or items.
        seed_session_thread(&sqlx, "thread-empty", "session-empty").await;

        backfill_thread(&sqlx, &rollout, "thread-empty", "session-empty", "test-owner")
            .await
            .expect("backfill empty");

        assert_eq!(
            sqlx.rollout_backfill_status("thread-empty").await.unwrap().as_deref(),
            Some("completed"),
        );
        // Rollout read is empty (only the SessionMeta header line is present,
        // which contributes no messages/items/states).
        let sqlx_arc = Arc::new(sqlx);
        let adapter = RolloutBackedAgentStore::new(
            Arc::clone(&sqlx_arc) as Arc<dyn AgentStorePort>,
            Arc::clone(&sqlx_arc) as Arc<dyn RolloutIndex>,
            Arc::clone(&rollout),
            None,
        );
        let messages = adapter.list_thread_messages("thread-empty").await.expect("messages");
        assert!(messages.is_empty(), "empty legacy thread → empty rollout");
        let items = adapter.list_turn_items("thread-empty").await.expect("items");
        assert!(items.is_empty(), "empty legacy thread → no items");
        // The file carries exactly the SessionMeta header line.
        let lines = read_rollout_lines(&rollout.resolve_path("thread-empty"));
        assert_eq!(lines.len(), 1, "only the SessionMeta header line");
        assert!(matches!(lines[0].item, RolloutItem::SessionMeta(_)));
    }

    // G3: a re-upsert passing the creation stamp (last_turn_index=0,
    // last_item_id=None, line_count=0) must NOT clobber the real values stamped
    // by a prior backfill. The ON CONFLICT preserves them via MAX/COALESCE.
    #[tokio::test]
    async fn mark_rollout_session_preserves_progress_on_re_upsert() {
        let sqlx = store().await;
        seed_session_thread(&sqlx, "thread-clobber", "session-clobber").await;

        // Stamp the real progress (as a backfill would).
        sqlx.mark_rollout_session(
            "thread-clobber",
            "session-clobber",
            "path",
            5,
            Some("real-item"),
            10,
            "completed",
        )
        .await
        .expect("stamp real progress");

        let (last_turn_index, last_item_id, line_count): (i64, Option<String>, i64) =
            sqlx::query_as(
                "SELECT last_turn_index, last_item_id, line_count \
                 FROM rollout_session_index WHERE thread_id = 'thread-clobber'",
            )
            .fetch_one(&sqlx.pool)
            .await
            .expect("read row");
        assert_eq!(last_turn_index, 5);
        assert_eq!(last_item_id.as_deref(), Some("real-item"));
        assert_eq!(line_count, 10);

        // A re-upsert (or any second call) passing the creation stamp must NOT
        // reset the real values.
        sqlx.mark_rollout_session(
            "thread-clobber",
            "session-clobber",
            "path",
            0,
            None,
            0,
            "completed",
        )
        .await
        .expect("re-upsert");

        let (last_turn_index, last_item_id, line_count): (i64, Option<String>, i64) =
            sqlx::query_as(
                "SELECT last_turn_index, last_item_id, line_count \
                 FROM rollout_session_index WHERE thread_id = 'thread-clobber'",
            )
            .fetch_one(&sqlx.pool)
            .await
            .expect("read row after re-upsert");
        assert_eq!(last_turn_index, 5, "G3: non-zero last_turn_index not clobbered by re-upsert");
        assert_eq!(
            last_item_id.as_deref(),
            Some("real-item"),
            "G3: real last_item_id not clobbered by re-upsert"
        );
        assert_eq!(line_count, 10, "G4(b): non-zero line_count not clobbered by re-upsert");
    }

    // ---- Slice D2b: backfill lease (CAS concurrency-safety + crash recovery) ----

    /// Read `(lease_owner, lease_expires_at)` for `thread_id`. Returns
    /// `(None, None)` when no `rollout_backfill_state` row exists.
    async fn lease_state(sqlx: &SqlxStore, thread_id: &str) -> (Option<String>, Option<String>) {
        sqlx::query_as::<_, (Option<String>, Option<String>)>(
            "SELECT lease_owner, lease_expires_at FROM rollout_backfill_state WHERE thread_id = ?1",
        )
        .bind(thread_id)
        .fetch_optional(&sqlx.pool)
        .await
        .expect("read lease state")
        .unwrap_or((None, None))
    }

    // (1) Two owners on an EXISTING row: the CAS UPDATE WHERE guard makes the
    // lease mutually exclusive. Mutation: drop the
    // `AND (lease_owner IS NULL OR lease_expires_at IS NULL OR lease_expires_at
    // < ?5)` guard from the UPDATE → the second owner's UPDATE overwrites A's
    // lease and ALSO reports acquired → the `!b` assert fails.
    #[tokio::test]
    async fn lease_cas_is_mutually_exclusive_for_existing_row() {
        let sqlx = store().await;
        seed_session_thread(&sqlx, "t", "s").await;
        sqlx.mark_backfill_state("t", "in_progress", 0, None).await.expect("seed state row");
        let a = sqlx
            .try_acquire_backfill_lease("t", "owner-A", BACKFILL_LEASE_TTL_SECS)
            .await
            .expect("acquire A");
        assert!(a, "first owner acquires the free lease");
        let b = sqlx
            .try_acquire_backfill_lease("t", "owner-B", BACKFILL_LEASE_TTL_SECS)
            .await
            .expect("acquire B");
        assert!(!b, "second owner denied — lease is mutually exclusive");
        let (owner, _) = lease_state(&sqlx, "t").await;
        assert_eq!(owner.as_deref(), Some("owner-A"), "DB still records owner A");
    }

    // (2) Two owners on a FRESH row (no prior state): the first INSERTs via ON
    // CONFLICT DO NOTHING, the second is denied. Mutation: drop the
    // `ON CONFLICT (thread_id) DO NOTHING` → the second INSERT errors on the
    // primary-key conflict → `.expect("acquire B")` panics.
    #[tokio::test]
    async fn lease_cas_is_mutually_exclusive_for_fresh_row() {
        let sqlx = store().await;
        seed_session_thread(&sqlx, "t", "s").await;
        let a = sqlx
            .try_acquire_backfill_lease("t", "owner-A", BACKFILL_LEASE_TTL_SECS)
            .await
            .expect("acquire A");
        assert!(a, "first owner INSERTs the row and acquires");
        let b = sqlx
            .try_acquire_backfill_lease("t", "owner-B", BACKFILL_LEASE_TTL_SECS)
            .await
            .expect("acquire B");
        assert!(!b, "second owner denied — ON CONFLICT DO NOTHING leaves A's row");
    }

    // (3) An expired lease is re-acquirable. Mutation: drop the
    // `OR lease_expires_at < ?5` clause from the UPDATE WHERE → the expired
    // lease is still treated as live → B is denied → the `b` assert fails.
    #[tokio::test]
    async fn lease_cas_reacquires_after_expiry() {
        let sqlx = store().await;
        seed_session_thread(&sqlx, "t", "s").await;
        sqlx.mark_backfill_state("t", "in_progress", 0, None).await.expect("seed state row");
        assert!(
            sqlx.try_acquire_backfill_lease("t", "owner-A", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("A acquire")
        );
        // Backdate owner-A's lease to the epoch — strictly expired.
        sqlx::query(
            "UPDATE rollout_backfill_state SET lease_expires_at = '1970-01-01T00:00:00Z' \
             WHERE thread_id = 't'",
        )
        .execute(&sqlx.pool)
        .await
        .expect("backdate expiry");
        let b = sqlx
            .try_acquire_backfill_lease("t", "owner-B", BACKFILL_LEASE_TTL_SECS)
            .await
            .expect("B acquire");
        assert!(b, "stale (expired) lease is re-acquired by another owner");
        let (owner, _) = lease_state(&sqlx, "t").await;
        assert_eq!(owner.as_deref(), Some("owner-B"), "DB now records owner B");
    }

    // (4) backfill_thread SKIPS a thread whose lease is held by a live owner
    // (does NOT rewrite, does NOT mark failed). Mutation: delete the
    // `false => { … return Ok(BackfillOutcome::Skipped); }` branch so a
    // not-acquired worker proceeds anyway → B rewrites →
    // `file_exists("t-skip")` becomes true → the assert fails.
    #[tokio::test]
    async fn backfill_thread_skips_thread_holding_live_lease() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        seed_session_thread(&sqlx, "t-skip", "s-skip").await;
        seed_message(&sqlx, "m", "t-skip", 0, &user_msg("x"), "2026-01-01T00:00:00Z").await;
        // Owner A holds a live lease.
        sqlx.mark_backfill_state("t-skip", "in_progress", 0, None).await.expect("seed state");
        assert!(
            sqlx.try_acquire_backfill_lease("t-skip", "owner-A", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("A acquire")
        );

        let outcome = backfill_thread(&sqlx, &rollout, "t-skip", "s-skip", "owner-B")
            .await
            .expect("skip is not an error");
        assert_eq!(outcome, BackfillOutcome::Skipped, "live lease → skipped, not failed");
        assert!(!rollout.file_exists("t-skip").await, "skip did NOT rewrite the file");
        // B's skip left A's lease intact (no clobber).
        let (owner, _) = lease_state(&sqlx, "t-skip").await;
        assert_eq!(owner.as_deref(), Some("owner-A"), "skip left the holder's lease intact");
    }

    // (5) backfill_thread releases its lease on success. Mutation: delete the
    // post-run `release_backfill_lease` call → lease_owner stays set → the
    // `owner.is_none()` assert fails.
    #[tokio::test]
    async fn backfill_thread_releases_lease_on_success() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        seed_session_thread(&sqlx, "t-rel", "s-rel").await;
        seed_message(&sqlx, "m", "t-rel", 0, &user_msg("x"), "2026-01-01T00:00:00Z").await;

        let outcome =
            backfill_thread(&sqlx, &rollout, "t-rel", "s-rel", "owner-A").await.expect("backfill");
        assert_eq!(outcome, BackfillOutcome::Completed);

        let (owner, expires) = lease_state(&sqlx, "t-rel").await;
        assert!(owner.is_none(), "lease_owner cleared after success");
        assert!(expires.is_none(), "lease_expires_at cleared after success");
        // The released lease is re-acquirable by another owner.
        assert!(
            sqlx.try_acquire_backfill_lease("t-rel", "owner-B", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("B acquire")
        );
    }

    // (6) backfill_all_threads processes candidates larger than one batch (250 >
    // 200) and completes ALL of them across multiple batches. Mutation: make
    // the batch loop break after the first batch (e.g. process only
    // `candidates.chunks(BACKFILL_BATCH_SIZE).take(1)`) → only 200/250
    // complete → the `succeeded == 250` assert fails.
    #[tokio::test]
    async fn backfill_all_threads_processes_candidates_in_batches() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        for i in 0..250 {
            let tid = format!("t-{i:03}");
            let sid = format!("s-{i:03}");
            seed_session_thread(&sqlx, &tid, &sid).await;
            seed_message(&sqlx, &format!("m-{i}"), &tid, 0, &user_msg("x"), "2026-01-01T00:00:00Z")
                .await;
        }
        let sqlx_arc = Arc::new(sqlx);
        let (succeeded, skipped, failed) =
            backfill_all_threads(Arc::clone(&sqlx_arc), Arc::clone(&rollout)).await;
        assert_eq!(succeeded, 250, "all candidates backfilled across batches");
        assert_eq!(skipped, 0);
        assert_eq!(failed, 0);
        // Sampling across the batch boundary (200/201).
        for sample in ["t-000", "t-199", "t-200", "t-249"] {
            assert_eq!(
                sqlx_arc.rollout_backfill_status(sample).await.unwrap().as_deref(),
                Some("completed"),
                "{sample} completed",
            );
        }
    }

    // (7) A crashed worker (acquired then never released) blocks a fresh worker
    // until the lease TTL elapses, after which the stale lease is re-acquired
    // and the thread is backfilled.
    #[tokio::test]
    async fn backfill_thread_recovers_after_prior_worker_crash() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));
        seed_session_thread(&sqlx, "t-crash", "s-crash").await;
        seed_message(&sqlx, "m", "t-crash", 0, &user_msg("x"), "2026-01-01T00:00:00Z").await;
        // Worker A acquires then "crashes" (never releases).
        sqlx.mark_backfill_state("t-crash", "in_progress", 0, None).await.expect("seed state");
        assert!(
            sqlx.try_acquire_backfill_lease("t-crash", "worker-A", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("A acquire")
        );

        // Worker B arrives BEFORE the TTL → still blocked.
        let outcome = backfill_thread(&sqlx, &rollout, "t-crash", "s-crash", "worker-B")
            .await
            .expect("no err");
        assert_eq!(
            outcome,
            BackfillOutcome::Skipped,
            "before TTL the crashed lease still blocks B"
        );
        assert!(!rollout.file_exists("t-crash").await, "B did not rewrite while A's lease live");

        // Simulate the TTL elapsing.
        sqlx::query(
            "UPDATE rollout_backfill_state SET lease_expires_at = '1970-01-01T00:00:00Z' \
             WHERE thread_id = 't-crash'",
        )
        .execute(&sqlx.pool)
        .await
        .expect("backdate expiry");

        // Worker C arrives AFTER expiry → re-acquires + backfills.
        let outcome = backfill_thread(&sqlx, &rollout, "t-crash", "s-crash", "worker-C")
            .await
            .expect("backfill C");
        assert_eq!(outcome, BackfillOutcome::Completed, "stale lease re-acquired after TTL");
        assert!(rollout.file_exists("t-crash").await, "C rewrote the file");
        assert_eq!(
            sqlx.rollout_backfill_status("t-crash").await.unwrap().as_deref(),
            Some("completed"),
        );
    }

    // (8) release_backfill_lease clears both lease columns.
    #[tokio::test]
    async fn release_backfill_lease_frees_the_lease() {
        let sqlx = store().await;
        seed_session_thread(&sqlx, "t", "s").await;
        sqlx.mark_backfill_state("t", "in_progress", 0, None).await.expect("seed state");
        assert!(
            sqlx.try_acquire_backfill_lease("t", "owner-A", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("acquire")
        );
        let (owner, expires) = lease_state(&sqlx, "t").await;
        assert!(owner.is_some() && expires.is_some(), "lease held before release");
        sqlx.release_backfill_lease("t").await.expect("release");
        let (owner, expires) = lease_state(&sqlx, "t").await;
        assert!(owner.is_none(), "lease_owner NULL after release");
        assert!(expires.is_none(), "lease_expires_at NULL after release");
    }

    // (9) backfill_lease_owner() yields a unique id per call (pid prefix + uuid).
    #[tokio::test]
    async fn backfill_lease_owner_is_unique_per_invocation() {
        let a = backfill_lease_owner();
        let b = backfill_lease_owner();
        assert_ne!(a, b, "each invocation gets a unique owner id");
        assert!(a.starts_with("backfill-pid") && b.starts_with("backfill-pid"));
    }

    // H1 (review HIGH): the lease CAS must serialize CONCURRENT workers. This
    // spawns N tokio tasks that race `try_acquire_backfill_lease` on the SAME
    // thread and asserts EXACTLY ONE returns true and the DB records that
    // winner. With the test store's pool-of-1 the workers serialize at the
    // pool, but the CAS UPDATE WHERE guard is still what decides the winner:
    // the first UPDATE sets the lease, every later UPDATE's WHERE sees it held
    // → 0 rows → false. Mutation: drop the CAS UPDATE WHERE guard → every
    // worker's UPDATE overwrites and reports acquired → wins == N → the
    // `wins == 1` assert fails. This is the false-green the review flagged:
    // without it the CAS never faced a concurrent contender.
    #[tokio::test]
    async fn lease_cas_serializes_concurrent_workers_exactly_one_winner() {
        let sqlx = Arc::new(store().await);
        seed_session_thread(&sqlx, "t-race", "s-race").await;
        // Pre-create the state row so the UPDATE (CAS) branch — not the INSERT
        // fallback — is the contested path.
        sqlx.mark_backfill_state("t-race", "in_progress", 0, None).await.expect("seed state");

        const N: usize = 8;
        let mut handles = Vec::new();
        for i in 0..N {
            let store = Arc::clone(&sqlx);
            handles.push(tokio::spawn(async move {
                store
                    .try_acquire_backfill_lease(
                        "t-race",
                        &format!("owner-{i}"),
                        BACKFILL_LEASE_TTL_SECS,
                    )
                    .await
            }));
        }
        let mut wins = 0usize;
        let mut winner: Option<String> = None;
        for (i, handle) in handles.into_iter().enumerate() {
            let acquired = handle.await.expect("task joined").expect("acquire ok");
            if acquired {
                wins += 1;
                winner = Some(format!("owner-{i}"));
            }
        }
        assert_eq!(wins, 1, "exactly one concurrent worker wins the lease");
        let (db_owner, _) = lease_state(&sqlx, "t-race").await;
        assert_eq!(db_owner, winner, "DB records the unique winner");
    }

    // M1 (review MEDIUM): a thread whose backfill FAILS must release ONLY its
    // own lease (so it is retryable) and must NOT touch another worker's live
    // lease on a different thread. Seeds a corrupt item_json that makes
    // backfill_thread_inner fail, plus a sibling thread whose lease is held by
    // owner-A; after the run, the failed thread's lease is released + marked
    // failed, while owner-A's lease is untouched. Mutation: remove the inner
    // Err-path `release_backfill_lease` in backfill_thread → the failed
    // thread's lease stays held → the `owner.is_none()` assert fails.
    #[tokio::test]
    async fn backfill_all_threads_failure_releases_own_lease_not_anothers() {
        let sqlx = store().await;
        let dir = rollout_dir();
        let rollout = Arc::new(RolloutFileStore::new(dir.path().to_owned()));

        // thread-fail: a candidate that will fail (valid JSON that does NOT
        // decode as a TurnItem — passes the column's json_valid CHECK but
        // trips the serde decode inside backfill_thread_inner).
        seed_session_thread(&sqlx, "t-fail", "s-fail").await;
        sqlx::query(
            "INSERT INTO agent_turn_items (id, thread_id, turn_index, seq, item_json, created_at) \
             VALUES ('bad-item', 't-fail', 0, 0, '{}', '2026-01-01T00:00:00Z')",
        )
        .execute(&sqlx.pool)
        .await
        .expect("seed undecodable item");

        // thread-other: a candidate whose lease is held by a DIFFERENT worker.
        seed_session_thread(&sqlx, "t-other", "s-other").await;
        seed_message(&sqlx, "om", "t-other", 0, &user_msg("y"), "2026-01-01T00:00:00Z").await;
        sqlx.mark_backfill_state("t-other", "in_progress", 0, None)
            .await
            .expect("seed other state");
        assert!(
            sqlx.try_acquire_backfill_lease("t-other", "owner-A", BACKFILL_LEASE_TTL_SECS)
                .await
                .expect("A acquire on other")
        );

        let sqlx_arc = Arc::new(sqlx);
        let (succeeded, skipped, failed) =
            backfill_all_threads(Arc::clone(&sqlx_arc), Arc::clone(&rollout)).await;

        assert_eq!(succeeded, 0, "no thread completed");
        assert_eq!(failed, 1, "thread-fail failed");
        assert_eq!(skipped, 1, "thread-other skipped (lease held by owner-A)");

        // thread-fail: marked failed + its OWN lease released (retryable).
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM rollout_backfill_state WHERE thread_id = 't-fail'",
        )
        .fetch_optional(&sqlx_arc.pool)
        .await
        .expect("read fail status");
        assert_eq!(status.as_deref(), Some("failed"), "failed thread stamped failed");
        let (owner, _) = lease_state(&sqlx_arc, "t-fail").await;
        assert!(owner.is_none(), "failed thread released its own lease (retryable)");

        // thread-other: owner-A's live lease UNTOUCHED by the sibling failure.
        let (owner, expires) = lease_state(&sqlx_arc, "t-other").await;
        assert_eq!(owner.as_deref(), Some("owner-A"), "sibling failure did not clear A's lease");
        assert!(expires.is_some(), "sibling failure did not clear A's expiry");
    }
}
