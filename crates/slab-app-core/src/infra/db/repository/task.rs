use super::AnyStore;
use crate::domain::models::{TaskPayloadEnvelope, TaskStatus};
use crate::infra::db::entities::TaskRecord;

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::future::Future;

type TaskRow = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    DateTime<Utc>,
    DateTime<Utc>,
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
        let result_data = encode_task_payload(record.result_data.as_deref());
        let mut tx = self.pool.begin().await?;
        super::insert_task_row(&mut tx, &record, result_data.as_deref()).await?;
        tx.commit().await?;
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
            )| {
                TaskRecord {
                    id,
                    task_type,
                    status: TaskStatus::from_stored(&status, "task repository"),
                    model_id,
                    input_data,
                    result_data: decode_task_payload(result_data),
                    error_msg,
                    core_task_id,
                    created_at,
                    updated_at,
                }
            },
        ))
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
                )| {
                    TaskRecord {
                        id,
                        task_type,
                        status: TaskStatus::from_stored(&status, "task repository"),
                        model_id,
                        input_data,
                        result_data: decode_task_payload(result_data),
                        error_msg,
                        core_task_id,
                        created_at,
                        updated_at,
                    }
                },
            )
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
}

fn decode_task_payload(raw: Option<String>) -> Option<String> {
    let raw = raw?;
    let Ok(envelope) = serde_json::from_str::<TaskPayloadEnvelope>(&raw) else {
        tracing::warn!("stored task payload is not an envelope; ignoring result_data");
        return None;
    };

    if envelope.version != TASK_PAYLOAD_VERSION || envelope.kind != TASK_PAYLOAD_KIND {
        tracing::warn!(
            kind = %envelope.kind,
            version = envelope.version,
            "stored task payload envelope is unsupported; ignoring result_data"
        );
        return None;
    }

    match envelope.data {
        Value::String(value) => Some(value),
        value => serde_json::to_string(&value).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskStore, decode_task_payload, encode_task_payload};
    use crate::domain::models::TaskStatus;
    use crate::infra::db::{AnyStore, TaskRecord};
    use chrono::Utc;
    use std::str::FromStr;

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
    fn decode_task_payload_rejects_legacy_json() {
        let raw = String::from(r#"{"text":"legacy"}"#);
        assert_eq!(decode_task_payload(Some(raw)), None);
    }

    #[test]
    fn decode_task_payload_rejects_wrong_envelope_kind() {
        let raw = String::from(r#"{"kind":"other","version":1,"data":{"text":"legacy"}}"#);
        assert_eq!(decode_task_payload(Some(raw)), None);
    }

    #[tokio::test]
    async fn insert_task_wraps_result_payload_envelope() {
        let store = new_store().await;
        let now = Utc::now();

        store
            .insert_task(TaskRecord {
                id: "task-1".to_owned(),
                task_type: "text".to_owned(),
                status: TaskStatus::Succeeded,
                model_id: None,
                input_data: None,
                result_data: Some(r#"{"text":"current"}"#.to_owned()),
                error_msg: None,
                core_task_id: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .expect("insert task");

        let raw: String = sqlx::query_scalar("SELECT result_data FROM tasks WHERE id = 'task-1'")
            .fetch_one(&store.pool)
            .await
            .expect("stored result data");
        assert_eq!(decode_task_payload(Some(raw)).as_deref(), Some(r#"{"text":"current"}"#));

        let record = store.get_task("task-1").await.expect("get task").expect("task exists");
        assert_eq!(record.result_data.as_deref(), Some(r#"{"text":"current"}"#));
    }

    #[tokio::test]
    async fn task_payload_migration_wraps_legacy_results() {
        let options =
            sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:").create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");
        sqlx::query("CREATE TABLE tasks (id TEXT PRIMARY KEY, result_data TEXT)")
            .execute(&pool)
            .await
            .expect("create tasks");
        sqlx::query(
            "INSERT INTO tasks (id, result_data) VALUES
                ('legacy-json', '{\"text\":\"legacy\"}'),
                ('legacy-text', 'plain text payload'),
                ('current-envelope', '{\"kind\":\"task_result\",\"version\":1,\"data\":{\"text\":\"current\"}}')",
        )
        .execute(&pool)
        .await
        .expect("insert tasks");

        sqlx::query(include_str!(
            "../../../../migrations/20260530010000_task_payload_envelopes.sql"
        ))
        .execute(&pool)
        .await
        .expect("run migration");

        let legacy_json: String =
            sqlx::query_scalar("SELECT result_data FROM tasks WHERE id = 'legacy-json'")
                .fetch_one(&pool)
                .await
                .expect("legacy json");
        let legacy_text: String =
            sqlx::query_scalar("SELECT result_data FROM tasks WHERE id = 'legacy-text'")
                .fetch_one(&pool)
                .await
                .expect("legacy text");
        let current: String =
            sqlx::query_scalar("SELECT result_data FROM tasks WHERE id = 'current-envelope'")
                .fetch_one(&pool)
                .await
                .expect("current envelope");

        assert_eq!(decode_task_payload(Some(legacy_json)).as_deref(), Some(r#"{"text":"legacy"}"#));
        assert_eq!(decode_task_payload(Some(legacy_text)).as_deref(), Some("plain text payload"));
        assert_eq!(decode_task_payload(Some(current)).as_deref(), Some(r#"{"text":"current"}"#));
    }

    async fn new_store() -> AnyStore {
        let options = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")
            .expect("sqlite options");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect in-memory db");
        let store = AnyStore { pool };

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

        store
    }
}
