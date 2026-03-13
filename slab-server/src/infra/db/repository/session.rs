use super::AnyStore;
use crate::infra::db::entities::ChatSession;
use std::future::Future;

pub trait SessionStore: Send + Sync + 'static {
    fn create_session(
        &self,
        session: ChatSession,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_sessions(&self) -> impl Future<Output = Result<Vec<ChatSession>, sqlx::Error>> + Send;
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

    async fn delete_session(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chat_sessions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
