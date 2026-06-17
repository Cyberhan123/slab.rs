use super::AnyStore;
use crate::infra::db::entities::ChatMessage;
use chrono::{DateTime, Utc};
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
        let mut tx = self.pool.begin().await?;
        // Ensure FK target exists for clients that send chat completions directly
        // without creating a session via `/v1/sessions` first.
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) \
             VALUES (?1, '', NULL, ?2, ?2) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&msg.session_id)
        .bind(&created_at)
        .execute(&mut *tx)
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
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn list_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>, sqlx::Error> {
        let rows: Vec<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
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
                created_at,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::ChatStore;
    use crate::infra::db::{AnyStore, ChatMessage};
    use crate::test_support::migrated_test_store;
    use chrono::Utc;

    async fn new_store() -> AnyStore {
        migrated_test_store().await
    }

    #[tokio::test]
    async fn append_message_rolls_back_auto_session_when_message_insert_fails() {
        let store = new_store().await;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at)
             VALUES ('existing-session', '', NULL, ?1, ?1)",
        )
        .bind(&now)
        .execute(&store.pool)
        .await
        .expect("insert existing session");
        sqlx::query(
            "INSERT INTO chat_messages (id, session_id, role, content, created_at)
             VALUES ('message-1', 'existing-session', 'user', 'old', ?1)",
        )
        .bind(&now)
        .execute(&store.pool)
        .await
        .expect("insert existing message");

        let error = store
            .append_message(ChatMessage {
                id: "message-1".to_owned(),
                session_id: "new-session".to_owned(),
                role: "user".to_owned(),
                content: "new".to_owned(),
                created_at: Utc::now(),
            })
            .await
            .expect_err("duplicate message id should fail");
        assert!(error.to_string().contains("UNIQUE constraint failed"));

        let new_session_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_sessions WHERE id = 'new-session'")
                .fetch_one(&store.pool)
                .await
                .expect("count new session");
        assert_eq!(new_session_count, 0);
    }
}
