use crate::entities::contexts::task::application::ports::TaskRepository;
use crate::entities::contexts::task::domain::TaskRecord;
use crate::entities::SqlxStore;
use chrono::{DateTime, Utc};

fn parse_rfc3339_or_now(raw: String, field: &'static str) -> DateTime<Utc> {
    raw.parse().unwrap_or_else(|e: chrono::ParseError| {
        tracing::warn!(raw = %raw, error = %e, field, "failed to parse task timestamp; using now");
        Utc::now()
    })
}

impl TaskRepository for SqlxStore {
    async fn insert_task(&self, record: TaskRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&record.id)
        .bind(&record.task_type)
        .bind(&record.status)
        .bind(&record.model_id)
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
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET status = ?1, result_data = ?2, error_msg = ?3, updated_at = ?4 WHERE id = ?5")
            .bind(status)
            .bind(result_data)
            .bind(error_msg)
            .bind(&updated_at)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, sqlx::Error> {
        let row: Option<(String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, String, String)> =
            sqlx::query_as("SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at FROM tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(
            |(
                id,
                task_type,
                status,
                model_id,
                input_data,
                result_data,
                error_msg,
                core_task_id,
                created_at,
                updated_at,
            )| TaskRecord {
                id,
                task_type,
                status,
                model_id,
                input_data,
                result_data,
                error_msg,
                core_task_id,
                created_at: parse_rfc3339_or_now(created_at, "created_at"),
                updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
            },
        ))
    }

    async fn list_tasks(&self, task_type: Option<&str>) -> Result<Vec<TaskRecord>, sqlx::Error> {
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            String,
            String,
        )> = if let Some(tt) = task_type {
            sqlx::query_as("SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at FROM tasks WHERE task_type = ?1 ORDER BY created_at DESC")
                .bind(tt)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as("SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at FROM tasks ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    task_type,
                    status,
                    model_id,
                    input_data,
                    result_data,
                    error_msg,
                    core_task_id,
                    created_at,
                    updated_at,
                )| TaskRecord {
                    id,
                    task_type,
                    status,
                    model_id,
                    input_data,
                    result_data,
                    error_msg,
                    core_task_id,
                    created_at: parse_rfc3339_or_now(created_at, "created_at"),
                    updated_at: parse_rfc3339_or_now(updated_at, "updated_at"),
                },
            )
            .collect())
    }

    async fn interrupt_running_tasks(&self) -> Result<u64, sqlx::Error> {
        let updated_at = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE tasks SET status = 'interrupted', updated_at = ?1 WHERE status IN ('pending', 'running')",
        )
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
