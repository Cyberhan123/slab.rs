use super::AnyStore;
use crate::domain::models::{TaskPayloadEnvelope, TaskStatus};
use crate::infra::db::entities::TaskRecord;

use chrono::Utc;
use serde_json::Value;
use std::future::Future;
use std::str::FromStr;

type TaskRow = (
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
);

const TASK_PAYLOAD_KIND: &str = "task_result";
const TASK_PAYLOAD_VERSION: u32 = 1;

pub trait TaskStore: Send + Sync + 'static {
    fn insert_task(
        &self,
        record: TaskRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn update_task_status(
        &self,
        id: &str,
        status: TaskStatus,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn update_task_status_if_active(
        &self,
        id: &str,
        status: TaskStatus,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<bool, sqlx::Error>> + Send;
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
            "INSERT INTO tasks (id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&record.id)
        .bind(&record.task_type)
        .bind(record.status.as_str())
        .bind(&record.model_id)
        .bind(&record.input_data)
        .bind(encode_task_payload(record.result_data.as_deref()))
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
        status: TaskStatus,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE tasks SET status = ?1, result_data = ?2, error_msg = ?3, updated_at = ?4 WHERE id = ?5",
        )
        .bind(status.as_str())
        .bind(encode_task_payload(result_data))
        .bind(error_msg)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_task_status_if_active(
        &self,
        id: &str,
        status: TaskStatus,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE tasks SET status = ?1, result_data = ?2, error_msg = ?3, updated_at = ?4 \
             WHERE id = ?5 AND status IN ('pending', 'running')",
        )
        .bind(status.as_str())
        .bind(encode_task_payload(result_data))
        .bind(error_msg)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, sqlx::Error> {
        let row: Option<TaskRow> =
            sqlx::query_as(
                "SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                 FROM tasks WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at)| {
            TaskRecord {
                id,
                task_type,
                status: decode_task_status(&status),
                model_id,
                input_data,
                result_data: decode_task_payload(result_data),
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
        let rows: Vec<TaskRow> = if let Some(tt) = task_type {
            sqlx::query_as(
                    "SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                     FROM tasks WHERE task_type = ?1 ORDER BY created_at DESC",
                )
                .bind(tt)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query_as(
                    "SELECT id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at \
                     FROM tasks ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows
            .into_iter()
            .map(|(id, task_type, status, model_id, input_data, result_data, error_msg, core_task_id, created_at, updated_at)| {
                TaskRecord {
                    id,
                    task_type,
                    status: decode_task_status(&status),
                    model_id,
                    input_data,
                    result_data: decode_task_payload(result_data),
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

fn encode_task_payload(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    let data = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.to_owned()));

    serde_json::to_string(&TaskPayloadEnvelope {
        kind: TASK_PAYLOAD_KIND.to_owned(),
        version: TASK_PAYLOAD_VERSION,
        data,
    })
    .ok()
    .or_else(|| Some(raw.to_owned()))
}

fn decode_task_payload(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let Ok(envelope) = serde_json::from_str::<TaskPayloadEnvelope>(&raw) else {
        return Some(raw);
    };

    if envelope.version != TASK_PAYLOAD_VERSION || envelope.kind.trim().is_empty() {
        return Some(raw);
    }

    match envelope.data {
        Value::String(value) => Some(value),
        value => serde_json::to_string(&value).ok().or(Some(raw)),
    }
}

fn decode_task_status(raw: &str) -> TaskStatus {
    TaskStatus::from_str(raw).unwrap_or_else(|_| {
        tracing::warn!(status = %raw, "unknown task status stored in repository; defaulting to failed");
        TaskStatus::Failed
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_task_payload, encode_task_payload};

    #[test]
    fn envelope_round_trips_json_payload() {
        let raw = r#"{"image":"data:image/png;base64,abc"}"#;
        let encoded = encode_task_payload(Some(raw)).expect("encoded payload");

        let decoded = decode_task_payload(Some(encoded)).expect("decoded payload");
        assert_eq!(decoded, raw);
    }

    #[test]
    fn envelope_round_trips_plain_text_payload() {
        let raw = "plain text payload";
        let encoded = encode_task_payload(Some(raw)).expect("encoded payload");

        let decoded = decode_task_payload(Some(encoded)).expect("decoded payload");
        assert_eq!(decoded, raw);
    }

    #[test]
    fn decode_task_payload_preserves_legacy_json() {
        let raw = String::from(r#"{"text":"legacy"}"#);
        let decoded = decode_task_payload(Some(raw.clone())).expect("decoded payload");
        assert_eq!(decoded, raw);
    }
}
