//! Read-only diagnostics queries for agent thread stats + failed tool calls
//! (INFRA-08). These feed the `/v1/system/diagnostics/agent-stats` endpoint and,
//! ultimately, the host `export_diagnostics` snapshot. Row types deliberately
//! carry only whitelist-safe fields (no message content, no tool arguments).

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

/// Whitelisted failed-tool-call row (tool name + error output only; no arguments).
pub(crate) struct FailedToolCallRow {
    pub(crate) tool_name: String,
    pub(crate) output: Option<String>,
}

impl AnyStore {
    /// Recent agent threads (newest first) with the latest turn index per thread
    /// resolved via `agent_thread_messages`.
    pub(crate) async fn list_recent_agent_thread_stats(
        &self,
        limit: i64,
    ) -> Result<Vec<AgentThreadStatRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT id, status, depth, completion_text, \
                    COALESCE((SELECT MAX(turn_index) FROM agent_thread_messages \
                              WHERE thread_id = agent_threads.id), 0) AS turn_index \
             FROM agent_threads \
             ORDER BY updated_at DESC \
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

    /// Recent failed tool calls (newest first). The `output` column carries the
    /// error text when `status = 'failed'`; arguments are never selected.
    pub(crate) async fn list_recent_failed_tool_calls(
        &self,
        limit: i64,
    ) -> Result<Vec<FailedToolCallRow>, AppCoreError> {
        let rows = sqlx::query(
            "SELECT tool_name, output \
             FROM agent_tool_calls \
             WHERE status = 'failed' \
             ORDER BY completed_at DESC \
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AppCoreError::Internal(format!("failed tool call query: {error}")))?;

        rows.into_iter()
            .map(|row| {
                Ok(FailedToolCallRow {
                    tool_name: row.try_get("tool_name").map_err(map_row_error)?,
                    output: row.try_get("output").map_err(map_row_error)?,
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
    async fn recent_thread_stats_and_failed_tool_calls_are_whitelist_safe() {
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
        // A thread with no messages ⇒ turn_index falls back to 0.
        sqlx::query(
            "INSERT INTO agent_threads (id, session_id, status, created_at, updated_at) \
             VALUES ('thread-b', 'diag-session', 'running', '2026-06-30T00:00:00Z', '2026-06-30T00:02:00Z')",
        )
        .execute(&pool)
        .await
        .expect("insert thread-b");
        // thread-a reached turn 3.
        sqlx::query(
            "INSERT INTO agent_thread_messages (id, thread_id, turn_index, role, content, created_at) \
             VALUES ('msg-1', 'thread-a', 3, 'user', '{}', '2026-06-30T00:00:30Z')",
        )
        .execute(&pool)
        .await
        .expect("insert message");

        // One failed + one completed tool call.
        sqlx::query(
            "INSERT INTO agent_tool_calls (id, thread_id, tool_name, status, output, created_at, completed_at) \
             VALUES ('call-1', 'thread-a', 'shell', 'failed', 'command exited 1', \
                     '2026-06-30T00:00:40Z', '2026-06-30T00:00:41Z')",
        )
        .execute(&pool)
        .await
        .expect("insert failed tool call");
        sqlx::query(
            "INSERT INTO agent_tool_calls (id, thread_id, tool_name, status, output, created_at, completed_at) \
             VALUES ('call-2', 'thread-a', 'read_file', 'completed', 'ok', \
                     '2026-06-30T00:00:42Z', '2026-06-30T00:00:43Z')",
        )
        .execute(&pool)
        .await
        .expect("insert completed tool call");

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

        let failed = store.list_recent_failed_tool_calls(50).await.expect("failed tool calls");
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].tool_name, "shell");
        assert_eq!(failed[0].output.as_deref(), Some("command exited 1"));
    }
}
