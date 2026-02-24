use crate::entities::{dao::TaskRecord, AnyStore};

use chrono::Utc;
use std::future::Future;

pub trait TaskStore: Send + Sync + 'static {
    fn insert_task(
        &self,
        record: TaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn set_core_task_id(
        &self,
        id: &str,
        core_task_id: i64,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_task(
        &self,
        id: &str,
    ) -> impl Future<Output = Result<Option<TaskRecord>, sqlx::Error>> + Send;
    fn list_tasks(
        &self,
        task_type: Option<&str>,
    ) -> impl Future<Output = Result<Vec<TaskRecord>, sqlx::Error>> + Send;
    fn interrupt_running_tasks(&self) -> impl Future<Output = Result<u64, sqlx::Error>> + Send;
}

impl TaskStore for AnyStore {
    async fn insert_task(&self, record: TaskRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO tasks (id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&record.id)
        .bind(&record.task_type)
        .bind(&record.status)
        .bind(&record.input_data)
        .bind(&record.result_data)
        .bind(&record.error_msg)
        .bind(record.core_task_id)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = ?1, result_data = ?2, error_msg = ?3, updated_at = ?4 WHERE id = ?5",
        )
        .bind(status)
        .bind(result_data)
        .bind(error_msg)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_core_task_id(&self, id: &str, core_task_id: i64) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET core_task_id = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(core_task_id)
            .bind(&updated_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, sqlx::Error> {
        let row: Option<(String, String, String, Option<String>, Option<String>, Option<String>, Option<i64>, String, String)> =
            sqlx::query_as(
                "SELECT id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                 FROM tasks WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at)| {
            TaskRecord {
                id,
                task_type,
                status,
                input_data,
                result_data,
                error_msg,
                core_task_id,
                created_at: created_at.parse().unwrap_or_else(|e: chrono::ParseError| {
                    tracing::warn!(raw = %created_at, error = %e, "failed to parse task created_at; using now");
                    Utc::now()
                }),
                updated_at: updated_at.parse().unwrap_or_else(|e: chrono::ParseError| {
                    tracing::warn!(raw = %updated_at, error = %e, "failed to parse task updated_at; using now");
                    Utc::now()
                }),
            }
        }))
    }

    async fn list_tasks(&self, task_type: Option<&str>) -> Result<Vec<TaskRecord>, sqlx::Error> {
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            String,
            String,
        )> = if let Some(tt) = task_type {
            sqlx::query_as(
                    "SELECT id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                     FROM tasks WHERE task_type = ?1 ORDER BY created_at DESC",
                )
                .bind(tt)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as(
                    "SELECT id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                     FROM tasks ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows
            .into_iter()
            .map(|(id, task_type, status, input_data, result_data, error_msg, core_task_id, created_at, updated_at)| {
                TaskRecord {
                    id,
                    task_type,
                    status,
                    input_data,
                    result_data,
                    error_msg,
                    core_task_id,
                    created_at: created_at.parse().unwrap_or_else(|e: chrono::ParseError| {
                        tracing::warn!(raw = %created_at, error = %e, "failed to parse task created_at; using now");
                        Utc::now()
                    }),
                    updated_at: updated_at.parse().unwrap_or_else(|e: chrono::ParseError| {
                        tracing::warn!(raw = %updated_at, error = %e, "failed to parse task updated_at; using now");
                        Utc::now()
                    }),
                }
            })
            .collect())
    }

    async fn interrupt_running_tasks(&self) -> Result<u64, sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE tasks SET status = 'interrupted', updated_at = ?1 \
             WHERE status IN ('pending', 'running')",
        )
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
