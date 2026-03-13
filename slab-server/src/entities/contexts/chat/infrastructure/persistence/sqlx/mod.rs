use crate::entities::contexts::chat::application::ports::ChatRepository;
use crate::entities::contexts::chat::domain::{ChatMessage, ChatSession};
use crate::entities::SqlxStore;
use chrono::{DateTime, Utc};

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> DateTime<Utc> {
    raw.parse().unwrap_or_else(|e: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %e, field, "failed to parse chat timestamp; using now");
        Utc::now()
    })
}

impl ChatRepository for SqlxStore {
    async fn create_session(&self, session: ChatSession) -> Result<(), sqlx::Error> {
        let created_at = session.created_at.to_rfc3339();
        let updated_at = session.updated_at.to_rfc3339();
        sqlx::query("INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)")
            .bind(&session.id)
            .bind(&session.name)
            .bind(&session.state_path)
            .bind(&created_at)
            .bind(&updated_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<ChatSession>, sqlx::Error> {
        let rows: Vec<(String, String, Option<String>, String, String)> = sqlx::query_as("SELECT id, name, state_path, created_at, updated_at FROM chat_sessions ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, name, state_path, created_at, updated_at)| ChatSession {
                    id,
                    name,
                    state_path,
                    created_at: parse_rfc3339_or_now(created_at, "created_at"),
                    updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
                },
            )
            .collect())
    }

    async fn delete_session(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chat_sessions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn append_message(&self, msg: ChatMessage) -> Result<(), sqlx::Error> {
        let created_at = msg.created_at.to_rfc3339();
        sqlx::query("INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) VALUES (?1, '', NULL, ?2, ?2) ON CONFLICT(id) DO NOTHING")
            .bind(&msg.session_id)
            .bind(&created_at)
            .execute(&self.pool)
            .await?;
        sqlx::query("INSERT INTO chat_messages (id, session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
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
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as("SELECT id, session_id, role, content, created_at FROM chat_messages WHERE session_id = ?1 ORDER BY created_at ASC")
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
                created_at: parse_rfc3339_or_now(created_at, "created_at"),
            })
            .collect())
    }
}
