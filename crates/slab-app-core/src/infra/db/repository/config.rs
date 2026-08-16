use super::AnyStore;
use std::future::Future;

pub trait ConfigStore: Send + Sync + 'static {
    fn get_config_value(
        &self,
        key: &str,
    ) -> impl Future<Output = Result<Option<String>, sqlx::Error>> + Send;
    fn set_config_entry(
        &self,
        key: &str,
        name: Option<&str>,
        value: &str,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

impl ConfigStore for AnyStore {
    async fn get_config_value(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM config_store WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(v,)| v))
    }

    async fn set_config_entry(
        &self,
        key: &str,
        name: Option<&str>,
        value: &str,
    ) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO config_store (key, name, value, updated_at) \
             VALUES (?1, COALESCE(NULLIF(TRIM(?2), ''), ?1), ?3, ?4) \
             ON CONFLICT(key) DO UPDATE \
             SET name = COALESCE(NULLIF(TRIM(?2), ''), config_store.name), \
                 value = ?3, \
                 updated_at = ?4",
        )
        .bind(key)
        .bind(name)
        .bind(value)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
