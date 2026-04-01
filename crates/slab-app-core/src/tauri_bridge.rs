//! Tauri IPC command bridge for slab-app-core.
//!
//! When the `tauri` feature is enabled, this module exposes the core
//! business-logic operations as Tauri commands so that the desktop frontend
//! can call them directly without going through HTTP.
//!
//! # Usage
//!
//! 1. Call [`init_state`] once inside `setup` so the managed state is ready.
//! 2. Include the `core_*` symbols in the single `tauri::generate_handler![]`
//!    call in your `lib.rs`:
//!
//! ```no_run
//! use slab_app_core::tauri_bridge::{core_health, core_list_models,
//!                                    core_list_sessions, core_list_tasks};
//!
//! tauri::Builder::default()
//!     .invoke_handler(tauri::generate_handler![
//!         core_health,
//!         core_list_models,
//!         core_list_sessions,
//!         core_list_tasks,
//!         // … other app commands …
//!     ])
//!     .setup(|app| {
//!         tauri::async_runtime::block_on(
//!             slab_app_core::tauri_bridge::init_state(app.handle())
//!         )?;
//!         Ok(())
//!     });
//! ```

use std::sync::Arc;

use tauri::Manager as _;

use crate::context::AppState;
use crate::error::AppCoreError;

// ── State initialisation ──────────────────────────────────────────────────────

/// Initialise the core [`AppState`] and register it with the Tauri app.
///
/// Call this once inside the `setup` hook, for example via
/// `tauri::async_runtime::block_on(init_state(app.handle()))`.
/// After this returns every `core_*` Tauri command will have access to the
/// fully initialised state.
pub async fn init_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::config::Config;
    use crate::infra::db::AnyStore;
    use crate::infra::rpc::gateway::GrpcGateway;
    use crate::infra::settings::SettingsProvider;
    use crate::model_auto_unload::ModelAutoUnloadManager;

    let mut cfg = Config::from_env();

    // If the database URL is still the default relative placeholder, resolve
    // it to an absolute platform-specific path so data persists across
    // working-directory changes.
    if cfg.database_url == "sqlite://slab.db?mode=rwc" {
        let base = dirs_next::config_dir()
            .ok_or("could not determine the user config directory; set SLAB_DATABASE_URL")?
            .join("Slab");
        tokio::fs::create_dir_all(&base).await?;
        let normalized = base.join("slab.db").to_string_lossy().replace('\\', "/");
        // SQLite URLs use "sqlite://" for absolute Unix paths ("/…") and
        // "sqlite:///" for Windows paths that don't start with "/" (e.g. "C:/…").
        let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
        cfg.database_url = format!("{prefix}{normalized}?mode=rwc");
    }

    // Ensure the settings directory exists.
    if let Some(parent) = cfg.settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let store = Arc::new(AnyStore::connect(&cfg.database_url).await?);
    let settings = Arc::new(SettingsProvider::load(cfg.settings_path.clone()).await?);
    let pmid = Arc::new(crate::domain::services::PmidService::load(Arc::clone(&settings)).await?);
    let grpc = Arc::new(GrpcGateway::connect_from_config(&cfg).await?);
    let model_auto_unload =
        Arc::new(ModelAutoUnloadManager::new(Arc::clone(&pmid), Arc::clone(&grpc)));

    let state = Arc::new(AppState::new(Arc::new(cfg), pmid, grpc, store, model_auto_unload));
    app.manage(state);
    Ok(())
}

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
    let models =
        state.services.model.list_models().await.map_err(|e: AppCoreError| e.to_string())?;
    serde_json::to_value(models).map_err(|e| e.to_string())
}

/// List all chat sessions.
#[tauri::command]
pub async fn core_list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let sessions =
        state.services.session.list_sessions().await.map_err(|e: AppCoreError| e.to_string())?;
    serde_json::to_value(sessions).map_err(|e| e.to_string())
}

/// List all tasks.
#[tauri::command]
pub async fn core_list_tasks(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let tasks = state
        .services
        .task_application
        .list_tasks(None)
        .await
        .map_err(|e: AppCoreError| e.to_string())?;
    serde_json::to_value(tasks).map_err(|e| e.to_string())
}
