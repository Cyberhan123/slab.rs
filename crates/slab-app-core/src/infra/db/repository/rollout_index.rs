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
}
