//! Rollout-session index queries on [`SqlxStore`] (Slice 5).
//!
//! These back the rollout read gate (`backfill_status`) and the D2a list
//! ghost-gate. They are inherent to the SQL store and exposed via the
//! app-core-internal [`RolloutIndex`] trait so the rollout adapter
//! (`RolloutBackedAgentStore`) can call them — and so tests can mock them. They
//! are deliberately NOT on the slab-agent `AgentStorePort` trait: slab-agent
//! must stay pure (no SQL, no rollout).
//!
//! Slice E removed the startup backfill (and dropped `rollout_backfill_state`),
//! so the trait no longer carries the backfill/lease methods; only the
//! `rollout_session_index`-backed read gate + new-thread mark remain.

use async_trait::async_trait;

use super::SqlxStore;

/// app-core-internal read/write surface over the `rollout_session_index` table.
///
/// Implemented by [`SqlxStore`] (production) and by in-memory mocks in tests.
/// Lives outside `AgentStorePort` so slab-agent stays pure.
#[async_trait]
pub trait RolloutIndex: Send + Sync {
    /// `backfill_status` for `thread_id`, or `None` when no index row exists.
    /// (`backfill_status` is now a rollout-native marker rather than a backfill
    /// tracker — the name is retained for column-name stability.)
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
    /// caller passes the real values; a re-`upsert_thread` passing the creation
    /// stamp `0`/`None` does NOT clobber them. `last_turn_index` + `line_count`
    /// use scalar `MAX(excluded, existing)` (monotonic); `last_item_id` uses
    /// `COALESCE(excluded, existing)`.
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
}
