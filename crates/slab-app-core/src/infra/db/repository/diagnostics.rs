//! Read-only diagnostics queries for agent thread stats (INFRA-08). These feed
//! the `/v1/system/diagnostics/agent-stats` endpoint and, ultimately, the host
//! `export_diagnostics` snapshot. Row types deliberately carry only whitelist-
//! safe fields (no message content, no tool arguments).
//!
//! The legacy `agent_thread_messages` table (the previous source of
//! `turn_index`) and the `agent_tool_calls` audit table were DROPPED. Turn index
//! now comes from `rollout_session_index.last_turn_index`; the failed-tool-call
//! list is deferred (tool failures are captured by the rollout `TurnItem`
//! stream, and a rollout-native diagnostics reader is not yet implemented —
//! the response field stays, returned empty).

use sqlx::Row;

use crate::error::AppCoreError;
use crate::infra::db::AnyStore;

/// Whitelisted recent-thread row (no message content / config / secret data).
pub(crate) struct AgentThreadStatRow {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) depth: u32,
    pub(crate) completion_text: Option<String>,
    pub(crate) turn_index: u32,
}

impl AnyStore {
    /// Recent agent threads (newest first) with the latest turn index per
    /// thread resolved via `rollout_session_index.last_turn_index` (the
    /// legacy `agent_thread_messages` source was dropped; rollout is the
    /// only conversation source).
    pub(crate) async fn list_recent_agent_thread_stats(
        &self,
        limit: i64,
    ) -> Result<Vec<AgentThreadStatRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT agent_threads.id, agent_threads.status, agent_threads.depth, \
                    agent_threads.completion_text, \
                    COALESCE(rollout_session_index.last_turn_index, 0) AS turn_index \
             FROM agent_threads \
             LEFT JOIN rollout_session_index \
               ON rollout_session_index.thread_id = agent_threads.id \
             ORDER BY agent_threads.updated_at DESC \
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppCoreError::Internal(format!("agent thread stats query: {error}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(AgentThreadStatRow {
                    id: row.try_get("id").map_err(map_row_error)?,
                    status: row.try_get("status").map_err(map_row_error)?,
                    depth: row.try_get::<i64, _>("depth").map_err(map_row_error)? as u32,
                    completion_text: row.try_get("completion_text").map_err(map_row_error)?,
                    turn_index: row.try_get::<i64, _>("turn_index").map_err(map_row_error)? as u32,
                })
            })
            .collect()
    }
}

fn map_row_error(error: sqlx::Error) -> AppCoreError {
    AppCoreError::Internal(format!("diagnostics row decode: {error}"))
}

#[cfg(test)]
mod tests {
    use super::AnyStore;
    use crate::test_support::migrated_test_pool;

    #[tokio::test]
    async fn recent_thread_stats_resolve_turn_index_from_rollout_index() {
        let pool = migrated_test_pool().await;

        sqlx::query(
            "INSERT INTO chat_sessions (id, created_at, updated_at) \
             VALUES ('diag-session', '2026-06-30T00:00:00Z', '2026-06-30T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert session");

        // A completed thread with a known termination reason in completion_text.
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, status, depth, completion_text, created_at, updated_at) \
             VALUES ('thread-a', 'diag-session', 'interrupted', 2, 'max_turns_reached', \
                     '2026-06-30T00:00:00Z', '2026-06-30T00:01:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert thread-a");
        // A thread with no rollout index row ⇒ turn_index falls back to 0.
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, status, created_at, updated_at) \
             VALUES ('thread-b', 'diag-session', 'running', '2026-06-30T00:00:00Z', '2026-06-30T00:02:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert thread-b");
        // thread-a reached turn 3 — recorded in the rollout index (the
        // legacy agent_thread_messages source was dropped).
        sqlx::query(
            "INSERT INTO rollout_session_index \
                (thread_id, session_id, file_path, line_count, last_turn_index, last_item_id, \
                 last_updated_at, created_at, backfill_status) \
             VALUES ('thread-a', 'diag-session', '/a.jsonl', 0, 3, NULL, \
                     '2026-06-30T00:00:30Z', '2026-06-30T00:00:30Z', 'completed')",
        )
        .execute(&pool)
        .await
        .expect("insert rollout index for thread-a");

        let store = AnyStore { pool };
        let threads = store.list_recent_agent_thread_stats(50).await.expect("thread stats");
        // Newest by updated_at first ⇒ thread-b, then thread-a.
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "thread-b");
        assert_eq!(threads[0].turn_index, 0);
        assert_eq!(threads[1].id, "thread-a");
        assert_eq!(threads[1].depth, 2);
        assert_eq!(threads[1].turn_index, 3);
        assert_eq!(threads[1].status, "interrupted");
        assert_eq!(threads[1].completion_text.as_deref(), Some("max_turns_reached"));
    }
}
