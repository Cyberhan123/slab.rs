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
    ) -> impl std::future::Future<Output = Result<(), sqlx::Error>> + Send;

    /// Update the `status` and `latency_ms` fields once the response is known.
    ///
    /// `status` is the HTTP status code stored as `i64` to match SQLite's
    /// `INTEGER` affinity (all HTTP status codes fit comfortably in i64).
    fn update_response(
        &self,
        id: Uuid,
        status: i64,
        latency_ms: i64,
    ) -> impl std::future::Future<Output = Result<(), sqlx::Error>> + Send;

    /// Retrieve a single record by trace ID.
    fn get(
        &self,
        id: Uuid,
    ) -> impl std::future::Future<Output = Result<Option<RequestRecord>, sqlx::Error>> + Send;
}
