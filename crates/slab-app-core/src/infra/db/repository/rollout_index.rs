//! Rollout-session index queries on [`SqlxStore`] (Slice 5).
//!
//! These back the rollout read gate (`backfill_status`) and the legacy-to-
//! rollout backfill. They are inherent to the SQL store and exposed via the
//! app-core-internal [`RolloutIndex`] trait so the rollout adapter
//! (`RolloutBackedAgentStore`) and the backfill task can call them — and so
//! tests can mock them. They are deliberately NOT on the slab-agent
//! `AgentStorePort` trait: slab-agent must stay pure (no SQL, no rollout).

use async_trait::async_trait;

use super::SqlxStore;

/// app-core-internal read/write surface over the `rollout_session_index` +
/// `rollout_backfill_state` tables (plus the legacy-data probe used to tell
/// brand-new threads from pre-migration legacy threads).
///
/// Implemented by [`SqlxStore`] (production) and by in-memory mocks in tests.
/// Lives outside `AgentStorePort` so slab-agent stays pure.
#[async_trait]
pub trait RolloutIndex: Send + Sync {
    /// `backfill_status` for `thread_id`, or `None` when no index row exists.
    async fn rollout_backfill_status(&self, thread_id: &str) -> sqlx::Result<Option<String>>;

    /// BATCH read of `(backfill_status, line_count)` for a set of thread ids
    /// (Slice D2a H1+M2). Returns a map keyed by `thread_id`; a thread with no
    /// index row is simply ABSENT from the map (callers treat absence as
    /// `(None, 0)`). One `IN (...)` query replaces K per-thread
    /// `rollout_backfill_status` calls on the list path.
    ///
    /// `line_count` is what lets the list ghost-gate distinguish a NEWBORN
    /// native thread (stamped `completed` + `line_count = 0` at creation, before
    /// the recorder's first append materializes the rollout file) from a TRUE
    /// GHOST (a thread that `completed` backfill with `line_count > 0` whose
    /// rollout file has since gone missing). See
    /// `RolloutBackedAgentStore::exclude_true_ghosts`.
    async fn rollout_backfill_progress_for(
        &self,
        thread_ids: &[String],
    ) -> sqlx::Result<std::collections::HashMap<String, (Option<String>, i64)>>;

    /// Upsert the `rollout_session_index` row, stamping `last_updated_at`.
    /// `created_at` is set only on insert (preserved across updates).
    ///
    /// `line_count` / `last_turn_index` / `last_item_id` are written on INSERT.
    /// On CONFLICT they are preserved against a smaller/NULL overwrite (G3): the
    /// backfill path passes the real values; a re-`upsert_thread` passing the
    /// creation stamp `0`/`None` does NOT clobber them. `last_turn_index` +
    /// `line_count` use scalar `MAX(excluded, existing)` (monotonic — the
    /// backfill's complete-line set includes everything that came before);
    /// `last_item_id` uses `COALESCE(excluded, existing)`.
    #[allow(clippy::too_many_arguments)]
    async fn mark_rollout_session(
        &self,
        thread_id: &str,
        session_id: &str,
        file_path: &str,
        last_turn_index: u32,
        last_item_id: Option<&str>,
        line_count: u32,
        backfill_status: &str,
    ) -> sqlx::Result<()>;

    /// `(thread_id, session_id)` for every thread not yet marked
    /// `backfill_status = 'completed'` — the candidate set for the startup
    /// backfill. Already-completed threads (legacy backfilled or new threads
    /// stamped at creation) are excluded.
    async fn list_thread_ids_for_backfill(&self) -> sqlx::Result<Vec<(String, String)>>;

    /// Update the detailed `rollout_backfill_state` progress row.
    /// `status = "in_progress"` stamps `started_at`; `"completed"` / `"failed"`
    /// stamp `completed_at` (and preserve an existing `started_at`).
    async fn mark_backfill_state(
        &self,
        thread_id: &str,
        status: &str,
        lines_written: u32,
        error: Option<&str>,
    ) -> sqlx::Result<()>;

    /// Atomically acquire (compare-and-swap) the backfill lease for `thread_id`.
    ///
    /// Returns `Ok(true)` when this caller now holds the lease (it may proceed
    /// with the backfill), `Ok(false)` when the lease is already held by another
    /// live owner (the caller must SKIP the thread).
    ///
    /// The CAS is concurrency-safe: the lease is taken only when no live owner
    /// holds it. "Live" = `lease_owner IS NULL` (never acquired, e.g. a fresh
    /// row created by `mark_backfill_state`) OR `lease_expires_at < now` (a
    /// previous owner crashed or stalled past `lease_ttl_secs`). A crashed worker
    /// therefore blocks re-backfill of its thread only until the TTL elapses
    /// (default `BACKFILL_LEASE_TTL_SECS = 900s`), after which the stale lease is
    /// silently re-acquired — an acceptable trade-off for an idempotent,
    /// best-effort background task.
    ///
    /// `lease_owner` is the worker identifier (unique per
    /// `backfill_all_threads` invocation); `lease_ttl_secs` bounds how long a
    /// crashed owner can stall recovery (saturates to one day; see the impl for
    /// the overflow-safe clamp). Times are stored as same-timezone RFC3339 UTC
    /// strings so lexical (`TEXT`) ordering equals chronological ordering (see
    /// the impl comment for the exact ordering argument).
    async fn try_acquire_backfill_lease(
        &self,
        thread_id: &str,
        lease_owner: &str,
        lease_ttl_secs: u64,
    ) -> sqlx::Result<bool>;

    /// Release the backfill lease for `thread_id` (clear `lease_owner` +
    /// `lease_expires_at`). Called when a backfill finishes — success OR failure
    /// — so a failed thread can be retried by a later worker without waiting for
    /// the TTL. No-op if no row exists.
    async fn release_backfill_lease(&self, thread_id: &str) -> sqlx::Result<()>;

    /// Whether `thread_id` has any rows in the legacy three conversation tables
    /// (`agent_thread_messages` / `agent_turn_states` / `agent_turn_items`).
    /// Used by the rollout adapter to distinguish a brand-new thread (no legacy
    /// data → safe to mark rollout-native at creation) from a pre-migration
    /// legacy thread (legacy data → leave to the backfill).
    async fn thread_has_legacy_data(&self, thread_id: &str) -> sqlx::Result<bool>;
}

#[async_trait]
impl RolloutIndex for SqlxStore {
    async fn rollout_backfill_status(&self, thread_id: &str) -> sqlx::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT backfill_status FROM rollout_session_index WHERE thread_id = ?1",
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(status,)| status))
    }

    async fn rollout_backfill_progress_for(
        &self,
        thread_ids: &[String],
    ) -> sqlx::Result<std::collections::HashMap<String, (Option<String>, i64)>> {
        if thread_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        // QueryBuilder is the sqlx-0.9-safe way to build a dynamic `IN (...)`
        // list (every caller value goes through push_bind — no interpolation).
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT thread_id, backfill_status, line_count FROM rollout_session_index \
             WHERE thread_id IN (",
        );
        let mut separated = query.separated(", ");
        for id in thread_ids {
            separated.push_bind(id);
        }
        query.push(')');
        let rows: Vec<(String, Option<String>, i64)> =
            query.build_query_as().fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|(id, status, lc)| (id, (status, lc))).collect())
    }

    #[allow(clippy::too_many_arguments)]
    async fn mark_rollout_session(
        &self,
        thread_id: &str,
        session_id: &str,
        file_path: &str,
        last_turn_index: u32,
        last_item_id: Option<&str>,
        line_count: u32,
        backfill_status: &str,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "INSERT INTO rollout_session_index \
             (thread_id, session_id, file_path, line_count, last_turn_index, last_item_id, \
              last_updated_at, created_at, backfill_status) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
                     ?7) \
             ON CONFLICT(thread_id) DO UPDATE SET \
               session_id=excluded.session_id, \
               file_path=excluded.file_path, \
               last_turn_index=MAX(excluded.last_turn_index, rollout_session_index.last_turn_index), \
               last_item_id=COALESCE(excluded.last_item_id, rollout_session_index.last_item_id), \
               line_count=MAX(excluded.line_count, rollout_session_index.line_count), \
               last_updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
               backfill_status=excluded.backfill_status",
        )
        .bind(thread_id)
        .bind(session_id)
        .bind(file_path)
        .bind(i64::from(line_count))
        .bind(i64::from(last_turn_index))
        .bind(last_item_id)
        .bind(backfill_status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_thread_ids_for_backfill(&self) -> sqlx::Result<Vec<(String, String)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, session_id FROM agent_threads \
             WHERE id NOT IN ( \
                 SELECT thread_id FROM rollout_session_index WHERE backfill_status = 'completed' \
             ) \
             ORDER BY created_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn mark_backfill_state(
        &self,
        thread_id: &str,
        status: &str,
        lines_written: u32,
        error: Option<&str>,
    ) -> sqlx::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        // started_at is stamped when entering in_progress; completed_at when
        // reaching a terminal state. On update, an existing started_at is
        // preserved (COALESCE) so a retry does not lose the original start.
        let (started_at, completed_at): (Option<&str>, Option<&str>) = match status {
            "in_progress" => (Some(now.as_str()), None),
            "completed" | "failed" => (None, Some(now.as_str())),
            _ => (None, None),
        };
        sqlx::query(
            "INSERT INTO rollout_backfill_state \
             (thread_id, status, lines_written, error, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(thread_id) DO UPDATE SET \
               status=excluded.status, \
               lines_written=excluded.lines_written, \
               error=excluded.error, \
               started_at=COALESCE(rollout_backfill_state.started_at, excluded.started_at), \
               completed_at=excluded.completed_at",
        )
        .bind(thread_id)
        .bind(status)
        .bind(i64::from(lines_written))
        .bind(error)
        .bind(started_at)
        .bind(completed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn thread_has_legacy_data(&self, thread_id: &str) -> sqlx::Result<bool> {
        let (exists,): (i64,) = sqlx::query_as(
            "SELECT EXISTS ( \
                SELECT 1 FROM agent_thread_messages WHERE thread_id = ?1 \
                UNION ALL \
                SELECT 1 FROM agent_turn_states WHERE thread_id = ?1 \
                UNION ALL \
                SELECT 1 FROM agent_turn_items WHERE thread_id = ?1 \
             )",
        )
        .bind(thread_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists != 0)
    }

    async fn try_acquire_backfill_lease(
        &self,
        thread_id: &str,
        lease_owner: &str,
        lease_ttl_secs: u64,
    ) -> sqlx::Result<bool> {
        // RFC3339 UTC strings stored as TEXT. Lexical (`TEXT`) ordering equals
        // chronological ordering here because every value is written by this
        // function in the SAME timezone (+00:00, the `Z`/`+00:00` suffix):
        // `'+' < '.'` < any digit, so values sharing the suffix sort by their
        // leading `YYYY-MM-DDTHH:MM:SS` fields (fixed width) then by the
        // fractional-seconds tail. chrono's `to_rfc3339` (AutoSi) does drop
        // trailing zeroes in the fractional part, but that tail-stripping is
        // monotonic (a smaller instant never sorts after a larger one), so the
        // `<` comparison stays well-defined. Both the stored `lease_expires_at`
        // and the comparison `now` use this format.
        let now = chrono::Utc::now();
        // Saturate the TTL to one day rather than `i64::MAX`: a `u64` near its
        // upper bound overflows `chrono::Duration::seconds` (which would panic
        // on debug builds / wrap on release). 86_400 s is a sane upper bound for
        // a background-task lease and keeps `expires` representable. (L2.)
        let ttl_secs = i64::try_from(lease_ttl_secs).unwrap_or(86_400).clamp(0, 86_400);
        let expires = now + chrono::Duration::seconds(ttl_secs);
        let now_str = now.to_rfc3339();
        let expires_str = expires.to_rfc3339();

        // (1) Take an EXISTING row whose lease is free or stale. The CAS guards
        // in the WHERE ensure only the worker that observes a free/stale lease
        // mutates it. SQLite WAL serializes writers (one writer at a time +
        // busy_timeout), so two concurrent UPDATEs run one after the other: the
        // first sets the lease, the second's WHERE then sees it held → 0 rows.
        let updated = sqlx::query(
            "UPDATE rollout_backfill_state \
             SET status='in_progress', lease_owner=?2, lease_expires_at=?3, \
                 started_at=COALESCE(started_at, ?4), completed_at=NULL, error=NULL \
             WHERE thread_id=?1 \
               AND (lease_owner IS NULL OR lease_expires_at IS NULL \
                    OR lease_expires_at < ?5)",
        )
        .bind(thread_id)
        .bind(lease_owner)
        .bind(&expires_str)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() > 0 {
            return Ok(true);
        }

        // (2) The row either does not exist OR a live lease is held. Try to
        // INSERT a fresh row; ON CONFLICT DO NOTHING means a pre-existing row
        // (held by another worker) is left untouched → rows_affected = 0 → we
        // report "not acquired". The INSERT ... ON CONFLICT DO NOTHING is the
        // tie-breaker for the "both workers saw no row" race: only one INSERT
        // materializes the row, the loser gets 0.
        let inserted = sqlx::query(
            "INSERT INTO rollout_backfill_state \
             (thread_id, status, lines_written, error, started_at, completed_at, \
              lease_owner, lease_expires_at) \
             VALUES (?1, 'in_progress', 0, NULL, ?2, NULL, ?3, ?4) \
             ON CONFLICT(thread_id) DO NOTHING",
        )
        .bind(thread_id)
        .bind(&now_str)
        .bind(lease_owner)
        .bind(&expires_str)
        .execute(&self.pool)
        .await?;
        Ok(inserted.rows_affected() > 0)
    }

    async fn release_backfill_lease(&self, thread_id: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE rollout_backfill_state SET lease_owner=NULL, lease_expires_at=NULL \
             WHERE thread_id=?1",
        )
        .bind(thread_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
