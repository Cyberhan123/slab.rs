use super::AnyStore;
use crate::domain::models::TaskStatus;
use crate::infra::db::TaskStore;
use crate::infra::db::entities::{ModelDownloadRecord, TaskRecord};
use chrono::{DateTime, Utc};
use std::future::Future;

type ModelDownloadRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn row_to_record(
    (
        task_id,
        model_id,
        source_key,
        repo_id,
        filename,
        hub_provider,
        status,
        error_msg,
        created_at,
        updated_at,
    ): ModelDownloadRow,
) -> ModelDownloadRecord {
    ModelDownloadRecord {
        task_id,
        model_id,
        source_key,
        repo_id,
        filename,
        hub_provider,
        status: TaskStatus::from_stored(&status, "model download repository"),
        error_msg,
        created_at,
        updated_at,
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
        source_key: &str,
    ) -> impl Future<Output = Result<Option<ModelDownloadRecord>, sqlx::Error>> + Send;
    fn list_model_downloads(
        &self,
    ) -> impl Future<Output = Result<Vec<ModelDownloadRecord>, sqlx::Error>> + Send;
    fn get_model_download(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Option<ModelDownloadRecord>, sqlx::Error>> + Send;
    fn update_model_download_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn restart_model_download_task(
        &self,
        task_id: &str,
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
        super::insert_task_row(&mut tx, &task, None).await?;

        let download_created_at = download.created_at.to_rfc3339();
        let download_updated_at = download.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO model_downloads (task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&download.task_id)
        .bind(&download.model_id)
        .bind(&download.source_key)
        .bind(&download.repo_id)
        .bind(&download.filename)
        .bind(&download.hub_provider)
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
        source_key: &str,
    ) -> Result<Option<ModelDownloadRecord>, sqlx::Error> {
        let row: Option<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             WHERE model_id = ?1 AND source_key = ?2 AND status IN ('pending', 'running') \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(model_id)
        .bind(source_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
    }

    async fn list_model_downloads(&self) -> Result<Vec<ModelDownloadRecord>, sqlx::Error> {
        let rows: Vec<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_record).collect())
    }

    async fn get_model_download(
        &self,
        task_id: &str,
    ) -> Result<Option<ModelDownloadRecord>, sqlx::Error> {
        let row: Option<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg, created_at, updated_at \
             FROM model_downloads \
             WHERE task_id = ?1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(row_to_record))
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

    async fn restart_model_download_task(&self, task_id: &str) -> Result<(), sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE tasks \
             SET status = ?1, result_data = NULL, error_msg = NULL, updated_at = ?2 \
             WHERE id = ?3",
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(&updated_at)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE model_downloads \
             SET status = ?1, error_msg = NULL, updated_at = ?2 \
             WHERE task_id = ?3",
        )
        .bind(TaskStatus::Pending.as_str())
        .bind(&updated_at)
        .bind(task_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn reconcile_model_downloads(&self) -> Result<(), sqlx::Error> {
        let rows: Vec<ModelDownloadRow> = sqlx::query_as(
            "SELECT task_id, model_id, source_key, repo_id, filename, hub_provider, status, error_msg, created_at, updated_at \
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
    use crate::domain::models::{
        CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION, CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION,
        TaskStatus, UnifiedModelKind, UnifiedModelStatus,
    };
    use crate::infra::db::{
        AnyStore, ModelDownloadRecord, ModelStore, TaskRecord, TaskStore, UnifiedModelRecord,
    };
    use crate::test_support::migrated_test_store;
    use chrono::Utc;

    async fn new_store() -> AnyStore {
        let store = migrated_test_store().await;
        let now = Utc::now();
        store
            .upsert_model(UnifiedModelRecord {
                id: "model-a".to_owned(),
                display_name: "Model A".to_owned(),
                kind: UnifiedModelKind::Local.as_str().to_owned(),
                backend_id: None,
                capabilities: "[]".to_owned(),
                status: UnifiedModelStatus::NotDownloaded.as_str().to_owned(),
                spec: "{}".to_owned(),
                runtime_presets: None,
                materialized_artifacts: "{}".to_owned(),
                selected_download_source: None,
                config_schema_version: CURRENT_STORED_MODEL_CONFIG_SCHEMA_VERSION as i64,
                config_policy_version: CURRENT_STORED_MODEL_CONFIG_POLICY_VERSION as i64,
                created_at: now,
                updated_at: now,
            })
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
            source_key: "hugging_face::repo/model::model.gguf".to_owned(),
            repo_id: "repo/model".to_owned(),
            filename: "model.gguf".to_owned(),
            hub_provider: Some("hf_hub".to_owned()),
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
            .get_active_model_download_for_source("model-a", "hugging_face::repo/model::model.gguf")
            .await
            .expect("lookup active download")
            .expect("active download exists");
        assert_eq!(active.task_id, "task-1");
    }

    #[tokio::test]
    async fn concurrent_active_model_download_inserts_keep_single_source_owner() {
        let store = new_store().await;
        let first_store = store.clone();
        let second_store = store.clone();

        let (first, second) = tokio::join!(
            async move {
                first_store
                    .insert_model_download_operation(
                        new_task_record("task-1"),
                        new_download_record("task-1"),
                    )
                    .await
            },
            async move {
                second_store
                    .insert_model_download_operation(
                        new_task_record("task-2"),
                        new_download_record("task-2"),
                    )
                    .await
            }
        );

        let successes =
            [first.is_ok(), second.is_ok()].into_iter().filter(|success| *success).count();
        assert_eq!(successes, 1);

        let active = store
            .get_active_model_download_for_source("model-a", "hugging_face::repo/model::model.gguf")
            .await
            .expect("lookup active download")
            .expect("active download exists");
        assert!(matches!(active.task_id.as_str(), "task-1" | "task-2"));
        let downloads = store.list_model_downloads().await.expect("list downloads");
        assert_eq!(downloads.len(), 1);
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
                .get_active_model_download_for_source(
                    "model-a",
                    "hugging_face::repo/model::model.gguf",
                )
                .await
                .expect("lookup active download")
                .is_none()
        );

        let downloads = store.list_model_downloads().await.expect("list model downloads");
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].status, TaskStatus::Failed);
        assert_eq!(downloads[0].error_msg.as_deref(), Some("network lost"));
    }

    #[tokio::test]
    async fn restart_model_download_task_resets_task_and_download_rows() {
        let store = new_store().await;
        let mut task = new_task_record("task-1");
        task.status = TaskStatus::Failed;
        task.result_data = Some(r#"{"progress":{"current":1}}"#.to_owned());
        task.error_msg = Some("network lost".to_owned());
        let mut download = new_download_record("task-1");
        download.status = TaskStatus::Failed;
        download.error_msg = Some("network lost".to_owned());

        store.insert_model_download_operation(task, download).await.expect("insert download");

        store.restart_model_download_task("task-1").await.expect("restart task");

        let task = store.get_task("task-1").await.expect("get task").expect("task exists");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.result_data.is_none());
        assert!(task.error_msg.is_none());

        let download = store
            .get_model_download("task-1")
            .await
            .expect("get download")
            .expect("download exists");
        assert_eq!(download.status, TaskStatus::Pending);
        assert!(download.error_msg.is_none());
    }

    #[tokio::test]
    async fn restart_model_download_task_keeps_active_source_unique() {
        let store = new_store().await;
        let mut failed_task = new_task_record("task-1");
        failed_task.status = TaskStatus::Failed;
        let mut failed_download = new_download_record("task-1");
        failed_download.status = TaskStatus::Failed;
        store
            .insert_model_download_operation(failed_task, failed_download)
            .await
            .expect("insert failed download");
        store
            .insert_model_download_operation(
                new_task_record("task-2"),
                new_download_record("task-2"),
            )
            .await
            .expect("insert active download");

        let error = store
            .restart_model_download_task("task-1")
            .await
            .expect_err("restart should conflict with active source");

        let message = error.to_string();
        assert!(message.contains("UNIQUE constraint failed"), "unexpected error: {message}");
    }
}
