use std::sync::Arc;

use tauri::Manager as _;

use slab_app_core::config::{Config, default_database_url};
use slab_app_core::context::AppState;
use slab_app_core::domain::services::PmidService;
use slab_app_core::infra::db::AnyStore;
use slab_app_core::infra::rpc::gateway::GrpcGateway;
use slab_app_core::infra::settings::migrate_legacy_settings_if_needed;
use slab_app_core::launch::ResolvedLaunchSpec;
use slab_app_core::model_auto_unload::ModelAutoUnloadManager;
use slab_app_core::runtime_supervisor::RuntimeSupervisorStatus;

/// Initialise the shared `slab-app-core` state for native IPC handlers.
pub async fn init_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    launch_spec: &ResolvedLaunchSpec,
    runtime_status: Arc<RuntimeSupervisorStatus>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cfg = Config::from_env();

    if cfg.database_url == "sqlite://slab.db?mode=rwc" {
        cfg.database_url = default_database_url();
    }

    launch_spec.apply_to_config(&mut cfg);

    if let Some(parent) = cfg.settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::create_dir_all(&cfg.model_config_dir).await?;
    tokio::fs::create_dir_all(&cfg.session_state_dir).await?;

    let store = Arc::new(AnyStore::connect(&cfg.database_url).await?);
    migrate_legacy_settings_if_needed(&cfg.settings_path, store.as_ref()).await?;
    let pmid = Arc::new(PmidService::load_from_path(cfg.settings_path.clone()).await?);
    let grpc = Arc::new(GrpcGateway::connect_from_config(&cfg).await?);
    let model_auto_unload = Arc::new(ModelAutoUnloadManager::new(
        Arc::clone(&pmid),
        Arc::clone(&grpc),
        Arc::clone(&runtime_status),
    ));

    let state = Arc::new(AppState::new(
        Arc::new(cfg),
        pmid,
        grpc,
        runtime_status,
        None,
        store,
        model_auto_unload,
    ));
    state.services.model.sync_model_packs_from_disk().await?;
    app.manage(state);

    Ok(())
}
