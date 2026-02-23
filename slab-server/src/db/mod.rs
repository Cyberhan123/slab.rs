//! Database abstraction layer.
//!
//! [`RequestStore`] defines the interface for persisting request audit records.
//! The default implementation is [`sqlite::SqliteStore`].  To swap to another
//! database (Postgres, MySQL, …), implement [`RequestStore`] for your new
//! type and change the concrete type in [`crate::state::AppState`].
//!
//! All trait methods use `impl Future` in their signatures (stable since Rust
//! 1.75) so no extra `async-trait` crate is required.

pub mod sqlite;

use std::future::Future;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A single row in the `request_log` table.
#[derive(Debug, Clone)]
pub struct RequestRecord {
    /// Trace ID that ties together request, processing, and response.
    pub id: Uuid,
    /// HTTP method, e.g. `"POST"`.
    pub method: String,
    /// Request path, e.g. `"/v1/chat/completions"`.
    pub path: String,
    /// HTTP status code; `None` until the response has been written.
    pub status: Option<i64>,
    /// Round-trip latency in milliseconds; `None` until response written.
    pub latency_ms: Option<i64>,
    /// Timestamp when the request arrived at the server.
    pub created_at: DateTime<Utc>,
}

/// Trait for persisting API request audit records.
///
/// Implement this trait to swap SQLite for another database backend without
/// touching any handler code.
pub trait RequestStore: Send + Sync + 'static {
    /// Persist a new request record.
    fn insert(
        &self,
        record: RequestRecord,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    /// Update the `status` and `latency_ms` fields once the response is known.
    ///
    /// `status` is the HTTP status code stored as `i64` to match SQLite's
    /// `INTEGER` affinity (all HTTP status codes fit comfortably in i64).
    fn update_response(
        &self,
        id: Uuid,
        status: i64,
        latency_ms: i64,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;

    /// Retrieve a single record by trace ID.
    fn get(
        &self,
        id: Uuid,
    ) -> impl Future<Output = Result<Option<RequestRecord>, sqlx::Error>> + Send;
}

// ── Tasks ─────────────────────────────────────────────────────────────────────

/// A row in the `tasks` table.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub status: String,
    pub input_data: Option<String>,
    pub result_data: Option<String>,
    pub error_msg: Option<String>,
    /// slab-core runtime `TaskId` (u64) for tasks submitted via `api::backend(...).run()`.
    /// `None` for server-only tasks (e.g. pure ffmpeg conversion, download).
    pub core_task_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub trait TaskStore: Send + Sync + 'static {
    fn insert_task(&self, record: TaskRecord) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn update_task_status(
        &self,
        id: &str,
        status: &str,
        result_data: Option<&str>,
        error_msg: Option<&str>,
    ) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn set_core_task_id(&self, id: &str, core_task_id: i64) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_task(&self, id: &str) -> impl Future<Output = Result<Option<TaskRecord>, sqlx::Error>> + Send;
    fn list_tasks(&self, task_type: Option<&str>) -> impl Future<Output = Result<Vec<TaskRecord>, sqlx::Error>> + Send;
    fn interrupt_running_tasks(&self) -> impl Future<Output = Result<u64, sqlx::Error>> + Send;
}

// ── Chat sessions ─────────────────────────────────────────────────────────────

/// A row in the `chat_sessions` table.
#[derive(Debug, Clone)]
pub struct ChatSession {
    pub id: String,
    pub name: String,
    pub state_path: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub trait SessionStore: Send + Sync + 'static {
    fn create_session(&self, session: ChatSession) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn get_session(&self, id: &str) -> impl Future<Output = Result<Option<ChatSession>, sqlx::Error>> + Send;
    fn list_sessions(&self) -> impl Future<Output = Result<Vec<ChatSession>, sqlx::Error>> + Send;
    fn update_session_state_path(&self, id: &str, path: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn delete_session(&self, id: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
}

// ── Chat messages ─────────────────────────────────────────────────────────────

/// A single message row in the `chat_messages` table.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    /// `"user"`, `"assistant"`, or `"system"`.
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

pub trait MessageStore: Send + Sync + 'static {
    fn append_message(&self, msg: ChatMessage) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_messages(&self, session_id: &str) -> impl Future<Output = Result<Vec<ChatMessage>, sqlx::Error>> + Send;
}

// ── Config store ──────────────────────────────────────────────────────────────

pub trait ConfigStore: Send + Sync + 'static {
    fn get_config_value(&self, key: &str) -> impl Future<Output = Result<Option<String>, sqlx::Error>> + Send;
    fn set_config_value(&self, key: &str, value: &str) -> impl Future<Output = Result<(), sqlx::Error>> + Send;
    fn list_config_values(&self) -> impl Future<Output = Result<Vec<(String, String)>, sqlx::Error>> + Send;
}

