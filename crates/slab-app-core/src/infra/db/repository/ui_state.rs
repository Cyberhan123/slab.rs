use super::AnyStore;
use crate::infra::db::entities::UiStateRecord;
use chrono::{DateTime, Utc};
use std::future::Future;

type UiStateRow = (String, String, DateTime<Utc>);

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
    UiStateRecord { key, value, updated_at }
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
    use crate::test_support::migrated_test_store;
    use chrono::Utc;

    #[tokio::test]
    async fn ui_state_store_round_trips_values() {
        let store = new_store().await;

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

    async fn new_store() -> AnyStore {
        migrated_test_store().await
    }
}
