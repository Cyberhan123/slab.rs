//! Tauri IPC command bridge for slab-app-core.
//!
//! When the `tauri` feature is enabled, this module exposes the core
//! business-logic operations as Tauri commands so that the desktop frontend
//! can call them directly without going through HTTP.
//!
//! Usage in slab-app/src-tauri/src/lib.rs:
//! ```no_run
//! slab_app_core::tauri_bridge::register(builder)
//! ```

use std::sync::Arc;

use crate::context::AppState;
use crate::error::AppCoreError;

// ── Commands ─────────────────────────────────────────────────────────────────

/// Return a simple health indicator for the embedded core.
#[tauri::command]
pub async fn core_health(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    let _ = state.inner();
    Ok(true)
}

/// List all registered AI models.
#[tauri::command]
pub async fn core_list_models(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    state
        .services
        .model
        .list_models()
        .await
        .map(|models| serde_json::to_value(models).unwrap_or_default())
        .map_err(|e: AppCoreError| e.to_string())
}

/// List all chat sessions.
#[tauri::command]
pub async fn core_list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    state
        .services
        .session
        .list_sessions()
        .await
        .map(|sessions| serde_json::to_value(sessions).unwrap_or_default())
        .map_err(|e: AppCoreError| e.to_string())
}

/// List all tasks.
#[tauri::command]
pub async fn core_list_tasks(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    state
        .services
        .task_application
        .list_tasks(None)
        .await
        .map(|tasks| serde_json::to_value(tasks).unwrap_or_default())
        .map_err(|e: AppCoreError| e.to_string())
}

// ── Registration ─────────────────────────────────────────────────────────────

/// Register all slab-app-core Tauri commands into the given builder.
///
/// Call this in your `slab-app` setup before building the Tauri app:
/// ```no_run
/// let builder = slab_app_core::tauri_bridge::register(tauri::Builder::default());
/// ```
pub fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        core_health,
        core_list_models,
        core_list_sessions,
        core_list_tasks,
    ])
}
