//! Shared application state injected into every Axum handler.

use std::sync::Arc;

use crate::db::sqlite::SqliteStore;

/// State shared across all HTTP handlers and the IPC listener.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Persistent request / response audit store.
    pub store: Arc<SqliteStore>,
}
