//! Shared application state injected into every Axum handler.

use std::sync::Arc;

use crate::config::Config;
use crate::db::sqlite::SqliteStore;

/// State shared across all HTTP handlers and the IPC listener.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Server configuration (env-derived).
    pub config: Arc<Config>,
    /// Persistent request / response audit store.
    pub store: Arc<SqliteStore>,
}
