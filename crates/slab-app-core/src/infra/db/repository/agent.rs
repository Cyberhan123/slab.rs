//! SQL-backed implementation of [`AgentStorePort`] for the shared [`SqlxStore`].

use async_trait::async_trait;
use slab_agent::port::ThreadStatus;
use slab_agent::port::{AgentStorePort, ThreadListFilter, ThreadSnapshot};

use super::SqlxStore;

fn parse_status(s: &str) -> ThreadStatus {
    s.parse::<ThreadStatus>().unwrap_or_else(|error| {
        tracing::warn!(
            raw = s,
            error = %error,
            "unknown agent thread status in database; defaulting to Errored"
        );
        ThreadStatus::Errored
    })
}

/// sqlx row type for the `agent_threads` table.
#[derive(sqlx::FromRow)]
struct AgentThreadRow {
    id: String,
    session_id: String,
    parent_id: Option<String>,
    depth: i64,
    status: String,
    role_name: Option<String>,
    config_json: String,
    completion_text: Option<String>,
    created_at: String,
    updated_at: String,
    archived_at: Option<String>,
}

impl TryFrom<AgentThreadRow> for ThreadSnapshot {
    type Error = slab_agent::AgentError;

    fn try_from(r: AgentThreadRow) -> Result<Self, Self::Error> {
        let depth = u32::try_from(r.depth).map_err(|error| {
            tracing::warn!(
                thread_id = %r.id,
                depth = r.depth,
                error = %error,
                "invalid agent thread depth in database"
            );
            slab_agent::AgentError::Store(format!(
                "invalid agent thread depth for '{}': {} ({})",
                r.id, r.depth, error
            ))
        })?;

        Ok(ThreadSnapshot {
            id: r.id,
            session_id: r.session_id,
            parent_id: r.parent_id,
            depth,
            status: parse_status(&r.status),
            role_name: r.role_name,
            config_json: r.config_json,
            completion_text: r.completion_text,
            created_at: r.created_at,
            updated_at: r.updated_at,
            archived_at: r.archived_at,
        })
    }
}

#[async_trait]
impl AgentStorePort for SqlxStore {
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_threads \
             (id, session_id, parent_id, depth, status, role_name, config_json, \
              completion_text, created_at, updated_at, archived_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
             ON CONFLICT(id) DO UPDATE SET \
               session_id=excluded.session_id, \
               parent_id=excluded.parent_id, \
               depth=excluded.depth, \
               status=excluded.status, \
               role_name=excluded.role_name, \
               config_json=excluded.config_json, \
               completion_text=excluded.completion_text, \
               created_at=agent_threads.created_at, \
               updated_at=excluded.updated_at, \
               archived_at=excluded.archived_at",
        )
        .bind(&snapshot.id)
        .bind(&snapshot.session_id)
        .bind(&snapshot.parent_id)
        .bind(i64::from(snapshot.depth))
        .bind(snapshot.status.to_string())
        .bind(&snapshot.role_name)
        .bind(&snapshot.config_json)
        .bind(&snapshot.completion_text)
        .bind(&snapshot.created_at)
        .bind(&snapshot.updated_at)
        .bind(&snapshot.archived_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, slab_agent::AgentError> {
        let row: Option<AgentThreadRow> = sqlx::query_as(
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at, archived_at \
             FROM agent_threads WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        row.map(ThreadSnapshot::try_from).transpose()
    }

    async fn list_session_threads(
        &self,
        session_id: &str,
    ) -> Result<Vec<ThreadSnapshot>, slab_agent::AgentError> {
        let rows: Vec<AgentThreadRow> = sqlx::query_as(
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at, archived_at \
             FROM agent_threads WHERE session_id = ?1 AND parent_id IS NULL \
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        rows.into_iter().map(ThreadSnapshot::try_from).collect()
    }

    async fn list_session_threads_filtered(
        &self,
        session_id: &str,
        filter: &ThreadListFilter,
    ) -> Result<Vec<ThreadSnapshot>, slab_agent::AgentError> {
        // Bind all client-influenced values (cursor + limit) — never interpolate.
        // `before`: a far-future sentinel makes `updated_at < ?` match everything
        //   when no cursor is supplied (RFC 3339 sorts lexicographically).
        // `limit`: SQLite treats `LIMIT -1` as no limit.
        let before = filter.before_updated_at.as_deref().unwrap_or("9999-12-31T23:59:59Z");
        let limit = filter.limit.map_or(-1i64, |limit| limit as i64);
        // `include_archived == false` hides threads soft-deleted via `thread/archive`.
        // Two static literals (no dynamic SQL) — sqlx 0.9's `SqlSafeStr` rejects
        // built strings, and both branches are compile-time constants.
        let sql = if filter.include_archived {
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at, archived_at \
             FROM agent_threads \
             WHERE session_id = ?1 AND parent_id IS NULL AND updated_at < ?2 \
             ORDER BY updated_at DESC, created_at DESC, id ASC \
             LIMIT ?3"
        } else {
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at, archived_at \
             FROM agent_threads \
             WHERE session_id = ?1 AND parent_id IS NULL AND updated_at < ?2 \
             AND archived_at IS NULL \
             ORDER BY updated_at DESC, created_at DESC, id ASC \
             LIMIT ?3"
        };
        let rows: Vec<AgentThreadRow> = sqlx::query_as(sql)
            .bind(session_id)
            .bind(before)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        rows.into_iter().map(ThreadSnapshot::try_from).collect()
    }

    async fn update_thread_status(
        &self,
        id: &str,
        status: ThreadStatus,
        completion_text: Option<&str>,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "UPDATE agent_threads SET status = ?1, completion_text = ?2, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE id = ?3",
        )
        .bind(status.to_string())
        .bind(completion_text)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    // Slice E.2: the conversation surface (`insert_thread_message` /
    // `list_thread_messages` / `upsert_turn_state`) was REMOVED from
    // `AgentStorePort` — the trait is now pure metadata. The legacy
    // `agent_thread_messages` / `agent_turn_states` / `agent_turn_items` tables
    // were DROPPED in Slice E; rollout is the sole conversation/turn source.
    // slab-agent emits conversation data via `EventMsg` (`MessageAppended` /
    // `TurnStateChanged`); the app-core observer lands it in rollout. The
    // `single_shot` Responses-API path writes out-of-band through the
    // app-core-internal `RolloutConversationStore::append_*` trait.

    async fn archive_thread(
        &self,
        id: &str,
        archived_at: Option<&str>,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "UPDATE agent_threads SET archived_at = ?1, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        )
        .bind(archived_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_depth_overflow_is_rejected_on_read() {
        let error = ThreadSnapshot::try_from(AgentThreadRow {
            id: "thread-1".to_owned(),
            session_id: "session-1".to_owned(),
            parent_id: None,
            depth: i64::from(u32::MAX) + 1,
            status: "running".to_owned(),
            role_name: None,
            config_json: "{}".to_owned(),
            completion_text: None,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            archived_at: None,
        })
        .expect_err("invalid depth should fail");
        assert!(error.to_string().contains("invalid agent thread depth"));
    }

    #[tokio::test]
    async fn archive_thread_sets_archived_at_and_hides_from_default_list() {
        let store = seeded_store().await;
        store.archive_thread("thread-1", Some("2026-02-01T00:00:00Z")).await.expect("archive");
        let snap = store.get_thread("thread-1").await.expect("get").expect("present");
        assert_eq!(snap.archived_at.as_deref(), Some("2026-02-01T00:00:00Z"));

        // Default filter excludes archived; opting in returns it.
        let hidden = store
            .list_session_threads_filtered(
                "session-1",
                &ThreadListFilter { limit: None, before_updated_at: None, include_archived: false },
            )
            .await
            .expect("list");
        assert!(hidden.is_empty(), "archived thread hidden by default");

        let shown = store
            .list_session_threads_filtered(
                "session-1",
                &ThreadListFilter { limit: None, before_updated_at: None, include_archived: true },
            )
            .await
            .expect("list");
        assert_eq!(shown.len(), 1, "archived thread visible when include_archived");
    }

    async fn seeded_store() -> SqlxStore {
        let store = SqlxStore::connect("sqlite::memory:").await.expect("store");
        let now = "2026-01-01T00:00:00Z".to_owned();
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, created_at, updated_at) \
             VALUES ('session-1', '', ?1, ?1)",
        )
        .bind(&now)
        .execute(&store.pool)
        .await
        .expect("session");
        store
            .upsert_thread(&ThreadSnapshot {
                id: "thread-1".to_owned(),
                session_id: "session-1".to_owned(),
                parent_id: None,
                depth: 0,
                status: ThreadStatus::Running,
                role_name: None,
                config_json: "{}".to_owned(),
                completion_text: None,
                created_at: now.clone(),
                updated_at: now,
                archived_at: None,
            })
            .await
            .expect("thread");
        store
    }
}
