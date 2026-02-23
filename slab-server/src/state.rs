//! Shared application state injected into every Axum handler.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::db::sqlite::SqliteStore;

/// Tracks in-flight tokio task abort handles, keyed by task ID.
pub struct TaskManager {
    handles: std::sync::Mutex<HashMap<String, tokio::task::AbortHandle>>,
}

impl std::fmt::Debug for TaskManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.handles.lock().map(|h| h.len()).unwrap_or(0);
        write!(f, "TaskManager({count} handles)")
    }
}

impl TaskManager {
    pub fn new() -> Self {
        Self { handles: std::sync::Mutex::new(HashMap::new()) }
    }

    pub fn insert(&self, id: impl Into<String>, handle: tokio::task::AbortHandle) {
        if let Ok(mut map) = self.handles.lock() {
            map.insert(id.into(), handle);
        }
    }

    /// Cancel and remove a task.  Returns `true` if the handle was found.
    pub fn cancel(&self, id: &str) -> bool {
        if let Ok(mut map) = self.handles.lock() {
            if let Some(h) = map.remove(id) {
                h.abort();
                return true;
            }
        }
        false
    }

    pub fn remove(&self, id: &str) {
        if let Ok(mut map) = self.handles.lock() {
            map.remove(id);
        }
    }
}

/// State shared across all HTTP handlers and the IPC listener.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Server configuration (env-derived).
    pub config: Arc<Config>,
    /// Persistent request / response audit store.
    pub store: Arc<SqliteStore>,
    /// Tracks abort handles for running async tasks.
    pub task_manager: Arc<TaskManager>,
}
