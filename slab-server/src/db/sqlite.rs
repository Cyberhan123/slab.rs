//! SQLite implementation of [`RequestStore`].
//!
//! Uses [`sqlx`] with the `sqlite` feature.  Migrations are run automatically
//! on startup via [`SqliteStore::connect`].
//!
//! # Migrations path
//!
//! `sqlx::migrate!("./migrations")` resolves the path **at compile time**
//! relative to `CARGO_MANIFEST_DIR` (the crate root), so the directory is
//! embedded into the binary.  The database file location is determined at
//! runtime by the `SLAB_DATABASE_URL` environment variable and is **not**
//! related to the current working directory at runtime.
//!
//! # Queries
//!
//! The `sqlx::query` (runtime-verified) form is used deliberately so that no
//! `DATABASE_URL` environment variable is needed at compile time.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{ChatSession, ConfigStore, RequestRecord, RequestStore, SessionStore, TaskRecord, TaskStore};

/// SQLite-backed request audit store.
#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Open (or create) the SQLite database at `url` and run pending migrations.
    ///
    /// `url` should be a sqlx-compatible SQLite URL, e.g. `"sqlite://slab.db"`
    /// or `"sqlite://:memory:"` for tests.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(url).await?;
        // Path is resolved relative to CARGO_MANIFEST_DIR at compile time.
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }
}

impl RequestStore for SqliteStore {
    async fn insert(&self, record: RequestRecord) -> Result<(), sqlx::Error> {
        let id = record.id.to_string();
        let created_at = record.created_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO request_log (id, method, path, status, latency_ms, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&record.method)
        .bind(&record.path)
        .bind(record.status)
        .bind(record.latency_ms)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_response(
        &self,
        id: Uuid,
        status: i64,
        latency_ms: i64,
    ) -> Result<(), sqlx::Error> {
        let id_str = id.to_string();
        sqlx::query(
            "UPDATE request_log SET status = ?1, latency_ms = ?2 WHERE id = ?3",
        )
        .bind(status)
        .bind(latency_ms)
        .bind(&id_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<RequestRecord>, sqlx::Error> {
        let id_str = id.to_string();
        let row: Option<(String, String, String, Option<i64>, Option<i64>, String)> =
            sqlx::query_as(
                "SELECT id, method, path, status, latency_ms, created_at \
                 FROM request_log WHERE id = ?1",
            )
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|(row_id, method, path, status, latency_ms, created_at)| {
            let parsed_id = row_id.parse::<Uuid>().unwrap_or(id);
            let parsed_dt = created_at
                .parse::<DateTime<Utc>>()
                .unwrap_or_else(|_| Utc::now());
            RequestRecord {
                id: parsed_id,
                method,
                path,
                status,
                latency_ms,
                created_at: parsed_dt,
            }
        }))
    }
}

// ── TaskStore ─────────────────────────────────────────────────────────────────

impl TaskStore for SqliteStore {
    async fn insert_task(&self, record: TaskRecord) -> Result<(), sqlx::Error> {
        let created_at = record.created_at.to_rfc3339();
        let updated_at = record.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO tasks (id, task_type, status, input_data, result_data, error_msg, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&record.id)
        .bind(&record.task_type)
        .bind(&record.status)
        .bind(&record.input_data)
        .bind(&record.result_data)
        .bind(&record.error_msg)
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

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, sqlx::Error> {
        let row: Option<(String, String, String, Option<String>, Option<String>, Option<String>, String, String)> =
            sqlx::query_as(
                "SELECT id, task_type, status, input_data, result_data, error_msg, created_at, updated_at \
                 FROM tasks WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(id, task_type, status, input_data, result_data, error_msg, created_at, updated_at)| {
            TaskRecord {
                id,
                task_type,
                status,
                input_data,
                result_data,
                error_msg,
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
        let rows: Vec<(String, String, String, Option<String>, Option<String>, Option<String>, String, String)> =
            if let Some(tt) = task_type {
                sqlx::query_as(
                    "SELECT id, task_type, status, input_data, result_data, error_msg, created_at, updated_at \
                     FROM tasks WHERE task_type = ?1 ORDER BY created_at DESC",
                )
                .bind(tt)
                .fetch_all(&self.pool)
                .await?
            } else {
                sqlx::query_as(
                    "SELECT id, task_type, status, input_data, result_data, error_msg, created_at, updated_at \
                     FROM tasks ORDER BY created_at DESC",
                )
                .fetch_all(&self.pool)
                .await?
            };
        Ok(rows
            .into_iter()
            .map(|(id, task_type, status, input_data, result_data, error_msg, created_at, updated_at)| {
                TaskRecord {
                    id,
                    task_type,
                    status,
                    input_data,
                    result_data,
                    error_msg,
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

// ── SessionStore ──────────────────────────────────────────────────────────────

impl SessionStore for SqliteStore {
    async fn create_session(&self, session: ChatSession) -> Result<(), sqlx::Error> {
        let created_at = session.created_at.to_rfc3339();
        let updated_at = session.updated_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO chat_sessions (id, name, state_path, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.state_path)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_session(&self, id: &str) -> Result<Option<ChatSession>, sqlx::Error> {
        let row: Option<(String, String, Option<String>, String, String)> =
            sqlx::query_as(
                "SELECT id, name, state_path, created_at, updated_at \
                 FROM chat_sessions WHERE id = ?1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(id, name, state_path, created_at, updated_at)| ChatSession {
            id,
            name,
            state_path,
            created_at: created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: updated_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
        }))
    }

    async fn list_sessions(&self) -> Result<Vec<ChatSession>, sqlx::Error> {
        let rows: Vec<(String, String, Option<String>, String, String)> =
            sqlx::query_as(
                "SELECT id, name, state_path, created_at, updated_at \
                 FROM chat_sessions ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, state_path, created_at, updated_at)| ChatSession {
                id,
                name,
                state_path,
                created_at: created_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
                updated_at: updated_at.parse().unwrap_or_else(|_| chrono::Utc::now()),
            })
            .collect())
    }

    async fn update_session_state_path(&self, id: &str, path: &str) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE chat_sessions SET state_path = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(path)
        .bind(&updated_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_session(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM chat_sessions WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── ConfigStore ───────────────────────────────────────────────────────────────

impl ConfigStore for SqliteStore {
    async fn get_config_value(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM config_store WHERE key = ?1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(v,)| v))
    }

    async fn set_config_value(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO config_store (key, value, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
        )
        .bind(key)
        .bind(value)
        .bind(&updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_config_values(&self) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM config_store ORDER BY key")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }
}
