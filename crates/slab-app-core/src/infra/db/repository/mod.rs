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

#[derive(Clone, Debug)]
pub struct SqlxStore {
    pub(crate) pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlxStore {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(url)?;
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
