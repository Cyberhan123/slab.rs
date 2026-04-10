use super::AnyStore;
use crate::domain::models::TaskStatus;
use crate::infra::db::TaskStore;
use crate::infra::db::entities::{ModelDownloadRecord, TaskRecord};
use chrono::Utc;
use std::future::Future;

type ModelDownloadRow = (String, String, String, String, String, Option<String>, String, String);

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> chrono::DateTime<Utc> {
    raw.parse().unwrap_or_else(|e: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %e, field, "failed to parse model download timestamp; using now");
        Utc::now()
    })
}

fn decode_task_status(raw: &str) -> TaskStatus {
    raw.parse::<TaskStatus>().unwrap_or_else(|_| {
        tracing::warn!(status = %raw, "unknown model download status stored in repository; defaulting to failed");
        TaskStatus::Failed
    })
}

fn row_to_record(
    (
        task_id,
        model_id,
        repo_id,
        filename,
        status,
        error_msg,
        created_at,
        updated_at,
    ): ModelDownloadRow,
) -> ModelDownloadRecord {
    ModelDownloadRecord {
        task_id,
        model_id,
        repo_id,
        filename,
        status: decode_task_status(&status),
        error_msg,
        created_at: parse_rfc3339_or_now(created_at, "created_at"),
        updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
    }
}

pub trait ModelDownloadStore: Send + Sync + 'static {
    fn insert_model_download_operation(
        &self,
        task: TaskRecord,
        download: ModelDownloadRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_active_model_download_for_source(
        &self,
        model_id: &str,
        repo_id: &str,
        filename: &str,
    ) -> impl Future<Output = Result<Option<ModelDownloadRecord>, sqlx::Error>> + Send;
    fn list_model_downloads(
        &self,
    ) -> impl Future<Output = Result<Vec<ModelDownloadRecord>, sqlx::Error>> + Send;
    fn update_model_download_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn reconcile_model_downloads(&self) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

impl ModelDownloadStore for AnyStore {
    async fn insert_model_download_operation(
        &self,
        task: TaskRecord,
        download: ModelDownloadRecord,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let task_created_at = task.created_at.to_rfc3339();
        let task_updated_at = task.updated_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&task.id)
        .bind(&task.task_type)
        .bind(task.status.as_str())
        .bind(&task.model_id)
        .bind(&task.input_data)
        .bind(Option::<String>::None)
        .bind(&task.error_msg)
        .bind(task.core_task_id)
        .bind(&task_created_at)
        .bind(&task_updated_at)
        .execute(&mut *tx)
        .await?;

        let download_created_at = download.created_at.to_rfc3339();
        let download_updated_at = download.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO model_downloads (task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&download.task_id)
        .bind(&download.model_id)
        .bind(&download.repo_id)
        .bind(&download.filename)
        .bind(download.status.as_str())
        .bind(&download.error_msg)
        .bind(&download_created_at)
        .bind(&download_updated_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn get_active_model_download_for_source(
        &self,
        model_id: &str,
        repo_id: &str,
        filename: &str,
    ) -> Result<Option<ModelDownloadRecord>, sqlx::Error> {
        let row: Option<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             WHERE model_id = ?1 AND repo_id = ?2 AND filename = ?3 AND status IN ('pending', 'running') \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(model_id)
        .bind(repo_id)
        .bind(filename)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn list_model_downloads(&self) -> Result<Vec<ModelDownloadRecord>, sqlx::Error> {
        let rows: Vec<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_record).collect())
    }

    async fn update_model_download_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        error_msg: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE model_downloads \
             SET status = ?1, error_msg = ?2, updated_at = ?3 \
             WHERE task_id = ?4",
        )
        .bind(status.as_str())
        .bind(error_msg)
        .bind(&updated_at)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn reconcile_model_downloads(&self) -> Result<(), sqlx::Error> {
        let rows: Vec<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, repo_id, filename, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             WHERE status IN ('pending', 'running')",
        )
        .fetch_all(&self.pool)
        .await?;

        for download in rows.into_iter().map(row_to_record) {
            let Some(task) = self.get_task(&download.task_id).await? else {
                self.update_model_download_status(
                    &download.task_id,
                    TaskStatus::Interrupted,
                    Some("task record missing during model download reconciliation"),
                )
                .await?;
                continue;
            };

            if matches!(task.status, TaskStatus::Pending | TaskStatus::Running) {
                continue;
            }

            self.update_model_download_status(
                &download.task_id,
                task.status,
                task.error_msg.as_deref(),
            )
            .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ModelDownloadStore;
    use crate::domain::models::TaskStatus;
    use crate::infra::db::{AnyStore, ModelDownloadRecord, TaskRecord, TaskStore};
    use chrono::Utc;
    use std::str::FromStr;

    async fn new_store() -> AnyStore {
        sqlx::any::install_default_drivers();
        let options =
            sqlx::any::AnyConnectOptions::from_str("sqlite::memory:").expect("sqlite options");
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");
        let store = AnyStore { pool };

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&store.pool)
            .await
            .expect("enable foreign keys");
        sqlx::query("CREATE TABLE IF NOT EXISTS models (id TEXT PRIMARY KEY)")
            .execute(&store.pool)
            .await
            .expect("create models table");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                core_task_id INTEGER,
                model_id TEXT,
                task_type TEXT NOT NULL,
                status TEXT NOT NULL,
                input_data TEXT,
                result_data TEXT,
                error_msg TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&store.pool)
        .await
        .expect("create tasks table");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_downloads (
                task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
                model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
                repo_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                status TEXT NOT NULL,
                error_msg TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&store.pool)
        .await
        .expect("create model_downloads table");
        sqlx::query(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_model_downloads_active_source
             ON model_downloads(model_id, repo_id, filename)
             WHERE status IN ('pending', 'running')",
        )
        .execute(&store.pool)
        .await
        .expect("create active download unique index");
        sqlx::query("INSERT INTO models (id) VALUES ('model-a')")
            .execute(&store.pool)
            .await
            .expect("insert model");

        store
    }

    fn new_task_record(id: &str) -> TaskRecord {
        let now = Utc::now();
        TaskRecord {
            id: id.to_owned(),
            task_type: "model_download".to_owned(),
            status: TaskStatus::Pending,
            model_id: Some("model-a".to_owned()),
            input_data: None,
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn new_download_record(task_id: &str) -> ModelDownloadRecord {
        let now = Utc::now();
        ModelDownloadRecord {
            task_id: task_id.to_owned(),
            model_id: "model-a".to_owned(),
            repo_id: "repo/model".to_owned(),
            filename: "model.gguf".to_owned(),
            status: TaskStatus::Pending,
            error_msg: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn active_model_downloads_are_unique_per_source() {
        let store = new_store().await;

        store
            .insert_model_download_operation(
                new_task_record("task-1"),
                new_download_record("task-1"),
            )
            .await
            .expect("insert first download");

        let error = store
            .insert_model_download_operation(
                new_task_record("task-2"),
                new_download_record("task-2"),
            )
            .await
            .expect_err("second active download should conflict");

        let message = error.to_string();
        assert!(message.contains("UNIQUE constraint failed"), "unexpected error: {message}");

        let active = store
            .get_active_model_download_for_source("model-a", "repo/model", "model.gguf")
            .await
            .expect("lookup active download")
            .expect("active download exists");
        assert_eq!(active.task_id, "task-1");
    }

    #[tokio::test]
    async fn reconcile_model_downloads_follows_task_terminal_state() {
        let store = new_store().await;

        store
            .insert_model_download_operation(
                new_task_record("task-1"),
                new_download_record("task-1"),
            )
            .await
            .expect("insert download");

        store
            .update_task_status("task-1", TaskStatus::Failed, None, Some("network lost"))
            .await
            .expect("mark task failed");

        store.reconcile_model_downloads().await.expect("reconcile model downloads");

        assert!(
            store
                .get_active_model_download_for_source("model-a", "repo/model", "model.gguf")
                .await
                .expect("lookup active download")
                .is_none()
        );

        let downloads = store.list_model_downloads().await.expect("list model downloads");
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].status, TaskStatus::Failed);
        assert_eq!(downloads[0].error_msg.as_deref(), Some("network lost"));
    }
}
