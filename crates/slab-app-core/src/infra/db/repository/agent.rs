//! SQL-backed implementation of [`AgentStorePort`] for the shared [`SqlxStore`].

use async_trait::async_trait;
use slab_agent::port::ThreadStatus;
use slab_agent::port::{
    AgentStorePort, ThreadMessageRecord, ThreadSnapshot, ToolCallRecord, TurnStateRecord,
};
use slab_types::agent::ToolCallStatus;
use slab_types::{ConversationMessage, ConversationMessageContent};

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
}

#[derive(sqlx::FromRow)]
struct AgentThreadMessageRow {
    id: String,
    thread_id: String,
    turn_index: i64,
    role: String,
    content: String,
    created_at: String,
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
        })
    }
}

impl AgentThreadMessageRow {
    fn into_record(self) -> Result<ThreadMessageRecord, slab_agent::AgentError> {
        let turn_index = u32::try_from(self.turn_index).map_err(|error| {
            tracing::warn!(
                message_id = %self.id,
                thread_id = %self.thread_id,
                turn_index = self.turn_index,
                error = %error,
                "invalid agent thread message turn index in database"
            );
            slab_agent::AgentError::Store(format!(
                "invalid agent thread message turn_index for '{}': {} ({})",
                self.id, self.turn_index, error
            ))
        })?;
        let Self { id, thread_id, turn_index: _, role, content, created_at } = self;
        let message =
            serde_json::from_str::<ConversationMessage>(&content).unwrap_or_else(|error| {
                tracing::warn!(
                    message_id = %id,
                    thread_id = %thread_id,
                    error = %error,
                    "failed to decode stored agent thread message content; preserving raw text"
                );
                ConversationMessage {
                    role,
                    content: ConversationMessageContent::Text(content),
                    name: None,
                    tool_call_id: None,
                    tool_calls: Vec::new(),
                }
            });

        Ok(ThreadMessageRecord { id, thread_id, turn_index, message, created_at })
    }
}

#[async_trait]
impl AgentStorePort for SqlxStore {
    async fn upsert_thread(&self, snapshot: &ThreadSnapshot) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_threads \
             (id, session_id, parent_id, depth, status, role_name, config_json, \
              completion_text, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
             ON CONFLICT(id) DO UPDATE SET \
               session_id=excluded.session_id, \
               parent_id=excluded.parent_id, \
               depth=excluded.depth, \
               status=excluded.status, \
               role_name=excluded.role_name, \
               config_json=excluded.config_json, \
               completion_text=excluded.completion_text, \
               created_at=agent_threads.created_at, \
               updated_at=excluded.updated_at",
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
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn get_thread(&self, id: &str) -> Result<Option<ThreadSnapshot>, slab_agent::AgentError> {
        let row: Option<AgentThreadRow> = sqlx::query_as(
            "SELECT id, session_id, parent_id, depth, status, role_name, \
             config_json, completion_text, created_at, updated_at \
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
             config_json, completion_text, created_at, updated_at \
             FROM agent_threads WHERE session_id = ?1 AND parent_id IS NULL \
             ORDER BY updated_at DESC, created_at DESC, id ASC",
        )
        .bind(session_id)
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

    async fn insert_tool_call(
        &self,
        record: &ToolCallRecord,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_tool_calls \
             (id, thread_id, tool_name, arguments, output, status, created_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&record.id)
        .bind(&record.thread_id)
        .bind(&record.tool_name)
        .bind(&record.arguments)
        .bind(&record.output)
        .bind(record.status.to_string())
        .bind(&record.created_at)
        .bind(&record.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn update_tool_call_status(
        &self,
        id: &str,
        status: ToolCallStatus,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query("UPDATE agent_tool_calls SET status = ?1 WHERE id = ?2")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn update_tool_call(
        &self,
        id: &str,
        output: Option<&str>,
        status: ToolCallStatus,
        completed_at: &str,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "UPDATE agent_tool_calls SET output = ?1, status = ?2, completed_at = ?3 \
             WHERE id = ?4",
        )
        .bind(output)
        .bind(status.to_string())
        .bind(completed_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn insert_thread_message(
        &self,
        record: &ThreadMessageRecord,
    ) -> Result<(), slab_agent::AgentError> {
        let content = serde_json::to_string(&record.message)
            .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        sqlx::query(
            "INSERT INTO agent_thread_messages \
             (id, thread_id, turn_index, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&record.id)
        .bind(&record.thread_id)
        .bind(i64::from(record.turn_index))
        .bind(&record.message.role)
        .bind(content)
        .bind(&record.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }

    async fn list_thread_messages(
        &self,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageRecord>, slab_agent::AgentError> {
        let rows: Vec<AgentThreadMessageRow> = sqlx::query_as(
            "SELECT id, thread_id, turn_index, role, content, created_at \
             FROM agent_thread_messages WHERE thread_id = ?1 \
             ORDER BY turn_index ASC, created_at ASC, id ASC",
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;

        rows.into_iter().map(AgentThreadMessageRow::into_record).collect()
    }

    async fn upsert_turn_state(
        &self,
        record: &TurnStateRecord,
    ) -> Result<(), slab_agent::AgentError> {
        sqlx::query(
            "INSERT INTO agent_turn_states \
             (thread_id, turn_index, status, input_messages_json, tool_specs_json, \
              llm_response_json, error, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(thread_id, turn_index) DO UPDATE SET \
               status=excluded.status, \
               input_messages_json=COALESCE(excluded.input_messages_json, agent_turn_states.input_messages_json), \
               tool_specs_json=COALESCE(excluded.tool_specs_json, agent_turn_states.tool_specs_json), \
               llm_response_json=COALESCE(excluded.llm_response_json, agent_turn_states.llm_response_json), \
               error=excluded.error, \
               started_at=agent_turn_states.started_at, \
               completed_at=COALESCE(excluded.completed_at, agent_turn_states.completed_at)",
        )
        .bind(&record.thread_id)
        .bind(i64::from(record.turn_index))
        .bind(&record.status)
        .bind(&record.input_messages_json)
        .bind(&record.tool_specs_json)
        .bind(&record.llm_response_json)
        .bind(&record.error)
        .bind(&record.started_at)
        .bind(&record.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| slab_agent::AgentError::Store(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn turn_state_upsert_preserves_existing_payload_fields() {
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
                updated_at: now.clone(),
            })
            .await
            .expect("thread");

        store
            .upsert_turn_state(&TurnStateRecord {
                thread_id: "thread-1".to_owned(),
                turn_index: 0,
                status: "running".to_owned(),
                input_messages_json: Some("[{\"role\":\"user\"}]".to_owned()),
                tool_specs_json: Some("[]".to_owned()),
                llm_response_json: None,
                error: None,
                started_at: now.clone(),
                completed_at: None,
            })
            .await
            .expect("running state");
        store
            .upsert_turn_state(&TurnStateRecord {
                thread_id: "thread-1".to_owned(),
                turn_index: 0,
                status: "completed".to_owned(),
                input_messages_json: None,
                tool_specs_json: None,
                llm_response_json: None,
                error: None,
                started_at: "ignored".to_owned(),
                completed_at: Some(now.clone()),
            })
            .await
            .expect("completed state");

        let row: (String, Option<String>, Option<String>, String, Option<String>) = sqlx::query_as(
            "SELECT status, input_messages_json, tool_specs_json, started_at, completed_at \
                 FROM agent_turn_states WHERE thread_id='thread-1' AND turn_index=0",
        )
        .fetch_one(&store.pool)
        .await
        .expect("state");

        assert_eq!(row.0, "completed");
        assert_eq!(row.1.as_deref(), Some("[{\"role\":\"user\"}]"));
        assert_eq!(row.2.as_deref(), Some("[]"));
        assert_eq!(row.3, now);
        assert_eq!(row.4.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[tokio::test]
    async fn malformed_thread_message_fallback_preserves_raw_content() {
        let store = seeded_store().await;
        sqlx::query(
            "INSERT INTO agent_thread_messages (id, thread_id, turn_index, role, content, created_at) \
             VALUES ('message-raw', 'thread-1', 0, 'assistant', 'not-json', '2026-01-01T00:00:00Z')",
        )
        .execute(&store.pool)
        .await
        .expect("insert raw message");

        let messages = store.list_thread_messages("thread-1").await.expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message.role, "assistant");
        assert_eq!(messages[0].message.content.rendered_text(), "not-json");
        assert!(messages[0].message.tool_calls.is_empty());
    }

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
        })
        .expect_err("invalid depth should fail");
        assert!(error.to_string().contains("invalid agent thread depth"));
    }

    #[test]
    fn thread_message_turn_index_overflow_is_rejected_on_read() {
        let error = AgentThreadMessageRow {
            id: "message-bad-index".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_index: i64::from(u32::MAX) + 1,
            role: "assistant".to_owned(),
            content: "{\"role\":\"assistant\",\"content\":\"ok\"}".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        }
        .into_record()
        .expect_err("invalid turn index should fail");
        assert!(error.to_string().contains("invalid agent thread message turn_index"));
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
            })
            .await
            .expect("thread");
        store
    }
}
