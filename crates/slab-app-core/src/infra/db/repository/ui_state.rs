use super::AnyStore;
use crate::infra::db::entities::UiStateRecord;
use chrono::Utc;
use std::future::Future;

type UiStateRow = (String, String, String);

pub trait UiStateStore: Send + Sync + 'static {
    fn upsert_ui_state(
        &self,
        record: UiStateRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_ui_state(
        &self,
        key: &str,
    ) -> impl Future<Output = Result<Option<UiStateRecord>, sqlx::Error>> + Send;
    fn delete_ui_state(&self, key: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

fn row_to_record((key, value, updated_at): UiStateRow) -> UiStateRecord {
    UiStateRecord {
        key,
        value,
        updated_at: updated_at.parse().unwrap_or_else(|error: chrono::ParseError| {
            tracing::warn!(raw = %updated_at, error = %error, "failed to parse ui_state updated_at; using now");
            Utc::now()
        }),
    }
}

impl UiStateStore for AnyStore {
    async fn upsert_ui_state(&self, record: UiStateRecord) -> Result<(), sqlx::Error> {
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO ui_state (\"key\", \"value\", updated_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(\"key\") DO UPDATE SET \
                \"value\" = excluded.\"value\", \
                updated_at = excluded.updated_at",
        )
        .bind(&record.key)
        .bind(&record.value)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_ui_state(&self, key: &str) -> Result<Option<UiStateRecord>, sqlx::Error> {
        let row: Option<UiStateRow> = sqlx::query_as(
            "SELECT \"key\", \"value\", updated_at \
             FROM ui_state WHERE \"key\" = ?1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn delete_ui_state(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM ui_state WHERE \"key\" = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::UiStateStore;
    use crate::infra::db::{AnyStore, UiStateRecord};
    use chrono::Utc;
    use std::str::FromStr;

    #[tokio::test]
    async fn ui_state_store_round_trips_values() {
        sqlx::any::install_default_drivers();
        let options =
            sqlx::any::AnyConnectOptions::from_str("sqlite::memory:").expect("sqlite options");
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");
        let store = AnyStore { pool };
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ui_state (
                \"key\" TEXT PRIMARY KEY,
                \"value\" TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&store.pool)
        .await
        .expect("create ui_state table");

        let now = Utc::now();
        store
            .upsert_ui_state(UiStateRecord {
                key: "zustand:chat-ui".to_owned(),
                value: "{\"state\":{\"currentSessionId\":\"session-1\"},\"version\":0}".to_owned(),
                updated_at: now,
            })
            .await
            .expect("upsert ui state");

        let record = store
            .get_ui_state("zustand:chat-ui")
            .await
            .expect("load ui state")
            .expect("ui state row exists");

        assert_eq!(record.key, "zustand:chat-ui");
        assert!(record.value.contains("session-1"));

        store.delete_ui_state("zustand:chat-ui").await.expect("delete ui state");

        assert!(store.get_ui_state("zustand:chat-ui").await.expect("load deleted row").is_none());
    }
}
