//! Tauri IPC commands that delegate directly to the embedded slab-app-core library.
//!
//! These commands replace the former `slab_app_core::tauri_bridge` module.
//! Because slab-app-core is now transport-agnostic, the Tauri-specific wiring
//! lives here in the desktop host binary.

use std::sync::Arc;

use tauri::Manager as _;

use slab_app_core::context::AppState;
use slab_app_core::error::AppCoreError;

// ── State initialisation ──────────────────────────────────────────────────────

/// Initialise the core [`AppState`] and register it with the Tauri app.
///
/// `runtime_grpc_endpoint` is the gRPC address of the embedded `slab-runtime`
/// sidecar, e.g. `"http://127.0.0.1:50051"`.  All three backends (llama,
/// whisper, diffusion) are expected to be served on the same endpoint.
///
/// Call this once inside the Tauri `setup` hook via
/// `tauri::async_runtime::block_on(init_core_state(app.handle(), endpoint))`.
pub async fn init_core_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    runtime_grpc_endpoint: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use slab_app_core::config::Config;
    use slab_app_core::infra::db::AnyStore;
    use slab_app_core::infra::rpc::gateway::GrpcGateway;
    use slab_app_core::infra::settings::SettingsProvider;
    use slab_app_core::model_auto_unload::ModelAutoUnloadManager;

    let mut cfg = Config::from_env();

    // Point all three gRPC backends at the single embedded slab-runtime sidecar.
    cfg.llama_grpc_endpoint = Some(runtime_grpc_endpoint.to_owned());
    cfg.whisper_grpc_endpoint = Some(runtime_grpc_endpoint.to_owned());
    cfg.diffusion_grpc_endpoint = Some(runtime_grpc_endpoint.to_owned());

    // Resolve the database URL to an absolute platform-specific path so data
    // persists across working-directory changes.
    if cfg.database_url == "sqlite://slab.db?mode=rwc" {
        let base = dirs_next::config_dir()
            .ok_or("could not determine the user config directory; set SLAB_DATABASE_URL")?
            .join("Slab");
        tokio::fs::create_dir_all(&base).await?;
        let normalized = base.join("slab.db").to_string_lossy().replace('\\', "/");
        let prefix = if normalized.starts_with('/') { "sqlite://" } else { "sqlite:///" };
        cfg.database_url = format!("{prefix}{normalized}?mode=rwc");
    }

    // Ensure the settings directory exists.
    if let Some(parent) = cfg.settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let store = Arc::new(AnyStore::connect(&cfg.database_url).await?);
    let settings = Arc::new(SettingsProvider::load(cfg.settings_path.clone()).await?);
    let pmid =
        Arc::new(slab_app_core::domain::services::PmidService::load(Arc::clone(&settings)).await?);
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
