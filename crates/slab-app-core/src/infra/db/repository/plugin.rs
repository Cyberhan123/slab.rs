use super::AnyStore;
use crate::infra::db::entities::PluginStateRecord;
use chrono::{DateTime, Utc};
use std::future::Future;

type PluginStateRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub trait PluginStateStore: Send + Sync + 'static {
    fn upsert_plugin_state(
        &self,
        record: PluginStateRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn get_plugin_state(
        &self,
        plugin_id: &str,
    ) -> impl Future<Output = Result<Option<PluginStateRecord>, sqlx::Error>> + Send;

    fn list_plugin_states(
        &self,
    ) -> impl Future<Output = Result<Vec<PluginStateRecord>, sqlx::Error>> + Send;

    fn update_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
        runtime_status: &str,
        updated_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn update_plugin_runtime_status(
        &self,
        plugin_id: &str,
        runtime_status: &str,
        last_error: Option<&str>,
        last_started_at: Option<DateTime<Utc>>,
        last_stopped_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    fn delete_plugin_state(
        &self,
        plugin_id: &str,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

impl PluginStateStore for AnyStore {
    async fn upsert_plugin_state(&self, record: PluginStateRecord) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO plugin_states (
                plugin_id, source_kind, source_ref, install_root, installed_version,
                manifest_hash, enabled, runtime_status, last_error, installed_at,
                updated_at, last_seen_at, last_started_at, last_stopped_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(plugin_id) DO UPDATE SET
                source_kind = excluded.source_kind,
                source_ref = COALESCE(excluded.source_ref, plugin_states.source_ref),
                install_root = COALESCE(excluded.install_root, plugin_states.install_root),
                installed_version = excluded.installed_version,
                manifest_hash = excluded.manifest_hash,
                runtime_status = excluded.runtime_status,
                last_error = excluded.last_error,
                updated_at = excluded.updated_at,
                last_seen_at = excluded.last_seen_at",
        )
        .bind(&record.plugin_id)
        .bind(&record.source_kind)
        .bind(&record.source_ref)
        .bind(&record.install_root)
        .bind(&record.installed_version)
        .bind(&record.manifest_hash)
        .bind(if record.enabled { 1_i64 } else { 0_i64 })
        .bind(&record.runtime_status)
        .bind(&record.last_error)
        .bind(record.installed_at.to_rfc3339())
        .bind(record.updated_at.to_rfc3339())
        .bind(record.last_seen_at.map(|value| value.to_rfc3339()))
        .bind(record.last_started_at.map(|value| value.to_rfc3339()))
        .bind(record.last_stopped_at.map(|value| value.to_rfc3339()))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_plugin_state(
        &self,
        plugin_id: &str,
    ) -> Result<Option<PluginStateRecord>, sqlx::Error> {
        let row: Option<PluginStateRow> = sqlx::query_as(PLUGIN_STATE_SELECT_WITH_ID)
            .bind(plugin_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(row_to_record))
    }

    async fn list_plugin_states(&self) -> Result<Vec<PluginStateRecord>, sqlx::Error> {
        let rows: Vec<PluginStateRow> =
            sqlx::query_as(PLUGIN_STATE_SELECT).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(row_to_record).collect())
    }

    async fn update_plugin_enabled(
        &self,
        plugin_id: &str,
        enabled: bool,
        runtime_status: &str,
        updated_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE plugin_states
             SET enabled = ?2, runtime_status = ?3, updated_at = ?4
             WHERE plugin_id = ?1",
        )
        .bind(plugin_id)
        .bind(if enabled { 1_i64 } else { 0_i64 })
        .bind(runtime_status)
        .bind(updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_plugin_runtime_status(
        &self,
        plugin_id: &str,
        runtime_status: &str,
        last_error: Option<&str>,
        last_started_at: Option<DateTime<Utc>>,
        last_stopped_at: Option<DateTime<Utc>>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE plugin_states
             SET runtime_status = ?2,
                 last_error = ?3,
                 last_started_at = COALESCE(?4, last_started_at),
                 last_stopped_at = COALESCE(?5, last_stopped_at),
                 updated_at = ?6
             WHERE plugin_id = ?1",
        )
        .bind(plugin_id)
        .bind(runtime_status)
        .bind(last_error)
        .bind(last_started_at.map(|value| value.to_rfc3339()))
        .bind(last_stopped_at.map(|value| value.to_rfc3339()))
        .bind(updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_plugin_state(&self, plugin_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM plugin_states WHERE plugin_id = ?1")
            .bind(plugin_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

const PLUGIN_STATE_SELECT: &str = "SELECT
    plugin_id, source_kind, source_ref, install_root, installed_version, manifest_hash,
    enabled, runtime_status, last_error, installed_at, updated_at, last_seen_at,
    last_started_at, last_stopped_at
    FROM plugin_states
    ORDER BY plugin_id";

const PLUGIN_STATE_SELECT_WITH_ID: &str = "SELECT
    plugin_id, source_kind, source_ref, install_root, installed_version, manifest_hash,
    enabled, runtime_status, last_error, installed_at, updated_at, last_seen_at,
    last_started_at, last_stopped_at
    FROM plugin_states
    WHERE plugin_id = ?1";

fn row_to_record(row: PluginStateRow) -> PluginStateRecord {
    PluginStateRecord {
        plugin_id: row.0,
        source_kind: row.1,
        source_ref: row.2,
        install_root: row.3,
        installed_version: row.4,
        manifest_hash: row.5,
        enabled: row.6 != 0,
        runtime_status: row.7,
        last_error: row.8,
        installed_at: parse_time("installed_at", &row.9),
        updated_at: parse_time("updated_at", &row.10),
        last_seen_at: row.11.as_deref().map(|value| parse_time("last_seen_at", value)),
        last_started_at: row.12.as_deref().map(|value| parse_time("last_started_at", value)),
        last_stopped_at: row.13.as_deref().map(|value| parse_time("last_stopped_at", value)),
    }
}

fn parse_time(field: &str, value: &str) -> DateTime<Utc> {
    value.parse().unwrap_or_else(|error: chrono::ParseError| {
        tracing::warn!(field, raw = %value, error = %error, "failed to parse plugin timestamp; using now");
        Utc::now()
    })
}

#[cfg(test)]
mod tests {
    use super::PluginStateStore;
    use crate::infra::db::{AnyStore, PluginStateRecord};
    use chrono::Utc;
    use std::str::FromStr;

    #[tokio::test]
    async fn plugin_state_store_round_trips_lifecycle_fields() {
        sqlx::any::install_default_drivers();
        let options =
            sqlx::any::AnyConnectOptions::from_str("sqlite::memory:").expect("sqlite options");
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");
        let store = AnyStore { pool };
        create_table(&store).await;

        let now = Utc::now();
        store
            .upsert_plugin_state(PluginStateRecord {
                plugin_id: "example-plugin".to_owned(),
                source_kind: "market_pack".to_owned(),
                source_ref: Some("default".to_owned()),
                install_root: Some("C:/Slab/plugins/example-plugin".to_owned()),
                installed_version: Some("0.1.0".to_owned()),
                manifest_hash: Some("abc123".to_owned()),
                enabled: true,
                runtime_status: "stopped".to_owned(),
                last_error: None,
                installed_at: now,
                updated_at: now,
                last_seen_at: Some(now),
                last_started_at: None,
                last_stopped_at: None,
            })
            .await
            .expect("upsert plugin state");

        store
            .update_plugin_runtime_status("example-plugin", "running", None, Some(now), None, now)
            .await
            .expect("mark running");

        let record = store
            .get_plugin_state("example-plugin")
            .await
            .expect("load plugin state")
            .expect("plugin state exists");

        assert_eq!(record.plugin_id, "example-plugin");
        assert_eq!(record.source_kind, "market_pack");
        assert!(record.enabled);
        assert_eq!(record.runtime_status, "running");
        assert!(record.last_started_at.is_some());

        store
            .update_plugin_enabled("example-plugin", false, "stopped", now)
            .await
            .expect("disable plugin");

        let disabled = store
            .get_plugin_state("example-plugin")
            .await
            .expect("load disabled plugin")
            .expect("plugin still exists");
        assert!(!disabled.enabled);
        assert_eq!(disabled.runtime_status, "stopped");

        store.delete_plugin_state("example-plugin").await.expect("delete state");
        assert!(
            store.get_plugin_state("example-plugin").await.expect("load deleted plugin").is_none()
        );
    }

    async fn create_table(store: &AnyStore) {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plugin_states (
                plugin_id TEXT PRIMARY KEY NOT NULL,
                source_kind TEXT NOT NULL,
                source_ref TEXT,
                install_root TEXT,
                installed_version TEXT,
                manifest_hash TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                runtime_status TEXT NOT NULL DEFAULT 'stopped',
                last_error TEXT,
                installed_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_seen_at TEXT,
                last_started_at TEXT,
                last_stopped_at TEXT
            )",
        )
        .execute(&store.pool)
        .await
        .expect("create plugin_states table");
    }
}
