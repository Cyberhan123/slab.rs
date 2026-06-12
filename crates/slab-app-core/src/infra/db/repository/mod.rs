pub mod agent;
pub mod chat;
pub mod config;
pub mod media_task;
pub mod model;
pub mod model_config_state;
pub mod model_download;
pub mod plugin;
pub mod session;
pub mod task;
pub mod ui_state;

pub use chat::ChatStore;
pub use media_task::MediaTaskStore;
pub use model::ModelStore;
pub use model_config_state::ModelConfigStateStore;
pub use model_download::ModelDownloadStore;
pub use plugin::PluginStateStore;
pub use session::SessionStore;
pub use task::TaskStore;
pub use ui_state::UiStateStore;

use crate::infra::db::entities::TaskRecord;
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SqlxStore {
    pub(crate) pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlxStore {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)?
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_millis(5_000));
        let pool = sqlx::SqlitePool::connect_with(options).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

pub type AnyStore = SqlxStore;

async fn insert_task_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    record: &TaskRecord,
    result_data: Option<&str>,
) -> Result<(), sqlx::Error> {
    let created_at = record.created_at.to_rfc3339();
    let updated_at = record.updated_at.to_rfc3339();
    sqlx::query(
        "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(&record.id)
    .bind(&record.task_type)
    .bind(record.status.as_str())
    .bind(&record.model_id)
    .bind(&record.input_data)
    .bind(result_data)
    .bind(&record.error_msg)
    .bind(record.core_task_id)
    .bind(&created_at)
    .bind(&updated_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_configures_sqlite_concurrency_pragmas() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let database_url = slab_types::sqlite_url_for_path(&temp_dir.path().join("slab.db"));
        let store = SqlxStore::connect(&database_url).await.expect("connect store");

        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&store.pool)
            .await
            .expect("journal mode");
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&store.pool)
            .await
            .expect("busy timeout");
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&store.pool)
            .await
            .expect("foreign keys");

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert_eq!(busy_timeout, 5_000);
        assert_eq!(foreign_keys, 1);
    }
}
