use crate::entities::{AnyStore, dao::ChatSession};
use chrono::{DateTime, Utc};
use std::future::Future;

pub trait SessionStore: Send + Sync + 'static {
    fn create_session(&self, session: ChatSession) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_session(&self, id: &str) -> impl Future<Output = Result<Option<ChatSession>, sqlx::Error>> + Send;
    fn list_sessions(&self) -> impl Future<Output = Result<Vec<ChatSession>, sqlx::Error>> + Send;
    fn update_session_state_path(&self, id: &str, path: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn delete_session(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

impl SessionStore for AnyStore {
    async fn create_session(&self, session: ChatSession) -> Result<(), sqlx::Error> {
        let created_at = session.created_at.to_rfc3339();
        let updated_at = session.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.state_path)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_session(&self, id: &str) -> Result<Option<ChatSession>, sqlx::Error> {
        let row: Option<(String, String, Option<String>, String, String)> = sqlx::query_as(
            "SELECT id, name, state_path, created_at, updated_at \
                 FROM chat_sessions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(id, name, state_path, created_at, updated_at)| ChatSession {
                id,
                name,
                state_path,
                created_at: created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: updated_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            },
        ))
    }

    async fn list_sessions(&self) -> Result<Vec<ChatSession>, sqlx::Error> {
        let rows: Vec<(String, String, Option<String>, String, String)> = sqlx::query_as(
            "SELECT id, name, state_path, created_at, updated_at \
                 FROM chat_sessions ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, name, state_path, created_at, updated_at)| ChatSession {
                    id,
                    name,
                    state_path,
                    created_at: created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                    updated_at: updated_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                },
            )
            .collect())
    }

    async fn update_session_state_path(&self, id: &str, path: &str) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE chat_sessions SET state_path = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(path)
            .bind(&updated_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_session(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chat_sessions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}