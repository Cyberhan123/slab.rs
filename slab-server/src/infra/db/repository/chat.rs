use super::AnyStore;
use crate::infra::db::entities::ChatMessage;
use chrono::Utc;
use std::future::Future;

pub trait ChatStore: Send + Sync + 'static {
    fn append_message(
        &self,
        msg: ChatMessage,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_messages(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Vec<ChatMessage>, sqlx::Error>> + Send;
}

impl ChatStore for AnyStore {
    async fn append_message(&self, msg: ChatMessage) -> Result<(), sqlx::Error> {
        let created_at = msg.created_at.to_rfc3339();
        // Ensure FK target exists for clients that send chat completions directly
        // without creating a session via `/v1/sessions` first.
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) \
             VALUES (?1, '', NULL, ?2, ?2) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&msg.session_id)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&msg.id)
        .bind(&msg.session_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, sqlx::Error> {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, session_id, role, content, created_at \
             FROM chat_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(id, session_id, role, content, created_at)| ChatMessage {
                id,
                session_id,
                role,
                content,
                created_at: created_at.parse().unwrap_or_else(|e: chrono::ParseError| {
                    tracing::warn!(raw = %created_at, error = %e, "failed to parse message created_at; using now");
                    Utc::now()
                }),
            })
            .collect())
    }
}
