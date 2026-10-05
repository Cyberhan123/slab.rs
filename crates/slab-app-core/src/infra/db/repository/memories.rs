//! Read-only diagnostics queries for the agent memory pipeline. These feed
//! the `/v1/system/diagnostics/memories` endpoint: phase1 output backlog and
//! failures per project, per-project phase2 watermarks and lock state, and
//! recent phase2 consolidation runs. Row types deliberately carry only
//! pipeline metadata — no memory content.

use sqlx::Row;

use crate::error::AppCoreError;
use crate::infra::db::AnyStore;

/// Phase1 output count for one (project, status) pair.
pub(crate) struct Phase1StatusCountRow {
    pub(crate) project_key: String,
    pub(crate) status: String,
    pub(crate) count: i64,
}

/// One `agent_memory_phase2_locks` row (per-project watermarks + lease).
pub(crate) struct Phase2LockRow {
    pub(crate) job_key: String,
    pub(crate) status: String,
    pub(crate) lease_owner: Option<String>,
    pub(crate) lease_until: Option<String>,
    pub(crate) claimed_watermark: Option<String>,
    pub(crate) completed_watermark: Option<String>,
    pub(crate) updated_at: String,
}

/// One `agent_memory_phase2_runs` row.
pub(crate) struct Phase2RunRow {
    pub(crate) id: String,
    pub(crate) project_key: String,
    pub(crate) status: String,
    pub(crate) claimed_watermark: Option<String>,
    pub(crate) completed_watermark: Option<String>,
    pub(crate) started_at: String,
    pub(crate) completed_at: Option<String>,
    pub(crate) error: Option<String>,
}

impl AnyStore {
    /// Phase1 output counts grouped by project and status — the backlog and
    /// failure surface (`pending`/`running` vs `failed` per project store).
    pub(crate) async fn list_memory_phase1_status_counts(
        &self,
    ) -> Result<Vec<Phase1StatusCountRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT project_key, status, COUNT(*) AS count \
             FROM agent_memory_phase1_outputs \
             GROUP BY project_key, status \
             ORDER BY project_key, status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppCoreError::Internal(format!("memory phase1 counts query: {error}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(Phase1StatusCountRow {
                    project_key: row.try_get("project_key").map_err(map_row_error)?,
                    status: row.try_get("status").map_err(map_row_error)?,
                    count: row.try_get("count").map_err(map_row_error)?,
                })
            })
            .collect()
    }

    /// All per-project phase2 lock rows: claimed/completed watermarks plus
    /// lease owner and expiry (diagnostics for stuck consolidations).
    pub(crate) async fn list_memory_phase2_locks(
        &self,
    ) -> Result<Vec<Phase2LockRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT job_key, status, lease_owner, lease_until, claimed_watermark, \
                    completed_watermark, updated_at \
             FROM agent_memory_phase2_locks \
             ORDER BY job_key",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppCoreError::Internal(format!("memory phase2 locks query: {error}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(Phase2LockRow {
                    job_key: row.try_get("job_key").map_err(map_row_error)?,
                    status: row.try_get("status").map_err(map_row_error)?,
                    lease_owner: row.try_get("lease_owner").map_err(map_row_error)?,
                    lease_until: row.try_get("lease_until").map_err(map_row_error)?,
                    claimed_watermark: row.try_get("claimed_watermark").map_err(map_row_error)?,
                    completed_watermark: row
                        .try_get("completed_watermark")
                        .map_err(map_row_error)?,
                    updated_at: row.try_get("updated_at").map_err(map_row_error)?,
                })
            })
            .collect()
    }

    /// Most recent phase2 consolidation runs, newest first.
    pub(crate) async fn list_recent_memory_phase2_runs(
        &self,
        limit: i64,
    ) -> Result<Vec<Phase2RunRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT id, project_key, status, claimed_watermark, completed_watermark, \
                    started_at, completed_at, error \
             FROM agent_memory_phase2_runs \
             ORDER BY started_at DESC \
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppCoreError::Internal(format!("memory phase2 runs query: {error}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(Phase2RunRow {
                    id: row.try_get("id").map_err(map_row_error)?,
                    project_key: row.try_get("project_key").map_err(map_row_error)?,
                    status: row.try_get("status").map_err(map_row_error)?,
                    claimed_watermark: row.try_get("claimed_watermark").map_err(map_row_error)?,
                    completed_watermark: row
                        .try_get("completed_watermark")
                        .map_err(map_row_error)?,
                    started_at: row.try_get("started_at").map_err(map_row_error)?,
                    completed_at: row.try_get("completed_at").map_err(map_row_error)?,
                    error: row.try_get("error").map_err(map_row_error)?,
                })
            })
            .collect()
    }
}

fn map_row_error(error: sqlx::Error) -> AppCoreError {
    AppCoreError::Internal(format!("memory diagnostics row decode: {error}"))
}

#[cfg(test)]
mod tests {
    use super::AnyStore;
    use crate::test_support::migrated_test_pool;

    async fn seed_thread(pool: &sqlx::SqlitePool, session_id: &str, thread_id: &str) {
        sqlx::query(
            "INSERT OR IGNORE INTO chat_sessions (id, created_at, updated_at) \
             VALUES (?1, '2026-10-04T00:00:00Z', '2026-10-04T00:00:00Z')",
        )
        .bind(session_id)
        .execute(pool)
        .await
        .expect("insert session");
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, status, created_at, updated_at) \
             VALUES (?1, ?2, 'completed', '2026-10-04T00:00:00Z', '2026-10-04T00:00:00Z')",
        )
        .bind(thread_id)
        .bind(session_id)
        .execute(pool)
        .await
        .expect("insert thread");
    }

    async fn seed_phase1_output(
        pool: &sqlx::SqlitePool,
        thread_id: &str,
        session_id: &str,
        status: &str,
        project_key: &str,
    ) {
        sqlx::query(
            "INSERT INTO agent_memory_phase1_outputs \
                (thread_id, session_id, status, project_key) \
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(thread_id)
        .bind(session_id)
        .bind(status)
        .bind(project_key)
        .execute(pool)
        .await
        .expect("insert phase1 output");
    }

    #[tokio::test]
    async fn memory_diagnostics_queries_group_count_and_order() {
        let pool = migrated_test_pool().await;

        seed_thread(&pool, "diag-mem-session", "thread-a").await;
        seed_thread(&pool, "diag-mem-session", "thread-b").await;
        seed_thread(&pool, "diag-mem-session", "thread-c").await;
        seed_phase1_output(&pool, "thread-a", "diag-mem-session", "succeeded", "proj-a").await;
        seed_phase1_output(&pool, "thread-b", "diag-mem-session", "failed", "proj-a").await;
        seed_phase1_output(&pool, "thread-c", "diag-mem-session", "pending", "proj-b").await;

        sqlx::query(
            "INSERT INTO agent_memory_phase2_locks \
                (job_key, lease_owner, lease_until, claimed_watermark, completed_watermark, \
                 status, updated_at) \
             VALUES ('proj-a', 'owner-1', NULL, '2026-10-04T01:00:00Z', \
                     '2026-10-03T01:00:00Z', 'idle', '2026-10-04T01:00:00Z'), \
                    ('proj-b', NULL, NULL, NULL, NULL, 'idle', '2026-10-04T02:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert locks");

        sqlx::query(
            "INSERT INTO agent_memory_phase2_runs \
                (id, project_key, status, claimed_watermark, completed_watermark, started_at, \
                 completed_at, error) \
             VALUES ('run-1', 'proj-a', 'succeeded', 'w-1', 'w-1', '2026-10-04T01:00:00Z', \
                     '2026-10-04T01:05:00Z', NULL), \
                    ('run-2', 'proj-b', 'failed', 'w-2', NULL, '2026-10-04T02:00:00Z', \
                     '2026-10-04T02:01:00Z', 'lease lost'), \
                    ('run-3', 'proj-a', 'succeeded', 'w-0', 'w-0', '2026-10-03T00:00:00Z', \
                     '2026-10-03T00:05:00Z', NULL)",
        )
        .execute(&pool)
        .await
        .expect("insert runs");

        let store = AnyStore { pool };

        let counts = store.list_memory_phase1_status_counts().await.expect("counts");
        assert_eq!(counts.len(), 3);
        assert_eq!(
            (counts[0].project_key.as_str(), counts[0].status.as_str()),
            ("proj-a", "failed")
        );
        assert_eq!(counts[0].count, 1);
        assert_eq!(
            (counts[1].project_key.as_str(), counts[1].status.as_str()),
            ("proj-a", "succeeded")
        );
        assert_eq!(
            (counts[2].project_key.as_str(), counts[2].status.as_str()),
            ("proj-b", "pending")
        );

        let locks = store.list_memory_phase2_locks().await.expect("locks");
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].job_key, "proj-a");
        assert_eq!(locks[0].lease_owner.as_deref(), Some("owner-1"));
        assert_eq!(locks[0].claimed_watermark.as_deref(), Some("2026-10-04T01:00:00Z"));
        assert_eq!(locks[0].completed_watermark.as_deref(), Some("2026-10-03T01:00:00Z"));
        assert_eq!(locks[1].lease_owner, None);
        assert_eq!(locks[1].claimed_watermark, None);

        let runs = store.list_recent_memory_phase2_runs(2).await.expect("runs");
        // Newest-first by started_at; the limit drops the oldest run.
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].id, "run-2");
        assert_eq!(runs[0].error.as_deref(), Some("lease lost"));
        assert_eq!(runs[0].completed_watermark, None);
        assert_eq!(runs[1].id, "run-1");
        assert_eq!(runs[1].error, None);
    }

    #[tokio::test]
    async fn memory_diagnostics_queries_on_empty_tables() {
        let pool = migrated_test_pool().await;
        let store = AnyStore { pool };
        assert!(store.list_memory_phase1_status_counts().await.expect("counts").is_empty());
        assert!(store.list_memory_phase2_locks().await.expect("locks").is_empty());
        assert!(store.list_recent_memory_phase2_runs(20).await.expect("runs").is_empty());
    }
}
