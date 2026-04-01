//! Tauri IPC commands that delegate directly to the embedded slab-app-core services.
//!
//! This module is compiled only when the `tauri` cargo feature is enabled.
//! It mirrors the handler surface of `bin/slab-server/src/api` so that the
//! Tauri desktop host can call the same business logic without going through HTTP.
//!
//! # Usage
//!
//! In the Tauri host binary (`bin/slab-app`):
//!
//! 1. Enable the feature in `Cargo.toml`:
//!    ```toml
//!    slab-app-core = { ..., features = ["tauri"] }
//!    ```
//! 2. Call [`init_state`] inside the Tauri `setup` hook.
//! 3. Register all commands with [`tauri::generate_handler!`]—list every
//!    `pub` function in this module that is annotated with `#[tauri::command]`.

use std::sync::Arc;

use tauri::Manager as _;

use crate::context::AppState;
use crate::error::AppCoreError;
use crate::schemas::backend::{
    BackendListResponse, BackendStatusResponse, BackendTypeQuery, DownloadLibRequest,
    ReloadLibRequest,
};
use crate::schemas::models::{
    CreateModelRequest, DownloadModelRequest, ImportModelConfigRequest, ListAvailableQuery,
    ListModelsQuery, LoadModelRequest, ModelStatusResponse, SwitchModelRequest,
    UnifiedModelResponse, UnloadModelRequest, UpdateModelRequest,
};
use crate::schemas::session::{CreateSessionRequest, MessageResponse, SessionResponse};
use crate::schemas::setup::{CompleteSetupRequest, SetupStatusResponse};
use crate::schemas::system::GpuStatusResponse;
use crate::schemas::tasks::{OperationAcceptedResponse, TaskResponse, TaskResultPayload};

// ── State initialisation ──────────────────────────────────────────────────────

/// Initialise the core [`AppState`] and register it with the Tauri app handle.
///
/// `runtime_grpc_endpoint` is the gRPC address of the embedded `slab-runtime`
/// sidecar, e.g. `"http://127.0.0.1:50051"`.  All three backends (llama,
/// whisper, diffusion) are expected to be served on the same endpoint.
///
/// Call this once inside the Tauri `setup` hook:
/// ```rust,ignore
/// tauri::async_runtime::block_on(slab_app_core::tauri_bridge::init_state(
///     app.handle(),
///     "http://127.0.0.1:50051",
/// ))
/// ```
pub async fn init_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    runtime_grpc_endpoint: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::config::Config;
    use crate::infra::db::AnyStore;
    use crate::infra::rpc::gateway::GrpcGateway;
    use crate::infra::settings::SettingsProvider;
    use crate::model_auto_unload::ModelAutoUnloadManager;

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
        Arc::new(crate::domain::services::PmidService::load(Arc::clone(&settings)).await?);
    let grpc = Arc::new(GrpcGateway::connect_from_config(&cfg).await?);
    let model_auto_unload =
        Arc::new(ModelAutoUnloadManager::new(Arc::clone(&pmid), Arc::clone(&grpc)));

    let state = Arc::new(AppState::new(Arc::new(cfg), pmid, grpc, store, model_auto_unload));
    app.manage(state);
    Ok(())
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn map_err(e: AppCoreError) -> String {
    e.to_string()
}

fn validate<T: validator::Validate>(v: T) -> Result<T, String> {
    v.validate().map_err(|e| e.to_string())?;
    Ok(v)
}

// ── Health ────────────────────────────────────────────────────────────────────

/// Return `true` if the embedded core is initialised and responding.
#[tauri::command]
pub async fn core_health(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    let _ = state.inner();
    Ok(true)
}

// ── Models ────────────────────────────────────────────────────────────────────

/// List all registered AI models (`GET /v1/models`).
#[tauri::command]
pub async fn core_list_models(
    state: tauri::State<'_, Arc<AppState>>,
    query: Option<ListModelsQuery>,
) -> Result<Vec<UnifiedModelResponse>, String> {
    let filter = query.unwrap_or_default().into();
    let models =
        state.services.model.list_models(filter).await.map_err(map_err)?;
    Ok(models.into_iter().map(Into::into).collect())
}

/// Create a new model record (`POST /v1/models`).
#[tauri::command]
pub async fn core_create_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: CreateModelRequest,
) -> Result<UnifiedModelResponse, String> {
    Ok(state.services.model.create_model(req.into()).await.map_err(map_err)?.into())
}

/// Import a model config (`POST /v1/models/import`).
#[tauri::command]
pub async fn core_import_model_config(
    state: tauri::State<'_, Arc<AppState>>,
    req: ImportModelConfigRequest,
) -> Result<UnifiedModelResponse, String> {
    Ok(state.services.model.import_model_config(req.into()).await.map_err(map_err)?.into())
}

/// Get a single model by ID (`GET /v1/models/{id}`).
#[tauri::command]
pub async fn core_get_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<UnifiedModelResponse, String> {
    Ok(state.services.model.get_model(&id).await.map_err(map_err)?.into())
}

/// Update a model (`PUT /v1/models/{id}`).
#[tauri::command]
pub async fn core_update_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
    req: UpdateModelRequest,
) -> Result<UnifiedModelResponse, String> {
    Ok(state.services.model.update_model(&id, req.into()).await.map_err(map_err)?.into())
}

/// Delete a model (`DELETE /v1/models/{id}`).
#[tauri::command]
pub async fn core_delete_model(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<serde_json::Value, String> {
    let view = state.services.model.delete_model(&id).await.map_err(map_err)?;
    Ok(serde_json::json!({ "id": view.id, "status": view.status }))
}

/// Load a model into the runtime (`POST /v1/models/load`).
#[tauri::command]
pub async fn core_load_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: LoadModelRequest,
) -> Result<ModelStatusResponse, String> {
    Ok(state.services.model.load_model(req.into()).await.map_err(map_err)?.into())
}

/// Unload a model from the runtime (`POST /v1/models/unload`).
#[tauri::command]
pub async fn core_unload_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: UnloadModelRequest,
) -> Result<ModelStatusResponse, String> {
    Ok(state.services.model.unload_model(req.into()).await.map_err(map_err)?.into())
}

/// Switch the active model (`POST /v1/models/switch`).
#[tauri::command]
pub async fn core_switch_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: SwitchModelRequest,
) -> Result<ModelStatusResponse, String> {
    Ok(state.services.model.switch_model(req.into()).await.map_err(map_err)?.into())
}

/// Start downloading a model file (`POST /v1/models/download`).
#[tauri::command]
pub async fn core_download_model(
    state: tauri::State<'_, Arc<AppState>>,
    req: DownloadModelRequest,
) -> Result<OperationAcceptedResponse, String> {
    Ok(state.services.model.download_model(req.into()).await.map_err(map_err)?.into())
}

/// List files available in a HuggingFace repo (`GET /v1/models/available`).
#[tauri::command]
pub async fn core_list_available_models(
    state: tauri::State<'_, Arc<AppState>>,
    query: ListAvailableQuery,
) -> Result<serde_json::Value, String> {
    let response = state.services.model.list_available_models(query.into()).await.map_err(map_err)?;
    Ok(serde_json::json!({ "repo_id": response.repo_id, "files": response.files }))
}

// ── Sessions ──────────────────────────────────────────────────────────────────

/// List all chat sessions (`GET /v1/sessions`).
#[tauri::command]
pub async fn core_list_sessions(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<SessionResponse>, String> {
    let sessions = state.services.session.list_sessions().await.map_err(map_err)?;
    Ok(sessions.into_iter().map(Into::into).collect())
}

/// Create a new chat session (`POST /v1/sessions`).
#[tauri::command]
pub async fn core_create_session(
    state: tauri::State<'_, Arc<AppState>>,
    req: CreateSessionRequest,
) -> Result<SessionResponse, String> {
    Ok(state.services.session.create_session(req.into()).await.map_err(map_err)?.into())
}

/// Delete a chat session (`DELETE /v1/sessions/{id}`).
#[tauri::command]
pub async fn core_delete_session(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<serde_json::Value, String> {
    state.services.session.delete_session(&id).await.map_err(map_err)
}

/// List messages in a chat session (`GET /v1/sessions/{id}/messages`).
#[tauri::command]
pub async fn core_list_session_messages(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<Vec<MessageResponse>, String> {
    let messages = state.services.session.list_session_messages(&id).await.map_err(map_err)?;
    Ok(messages.into_iter().map(Into::into).collect())
}

// ── Tasks ─────────────────────────────────────────────────────────────────────

/// List all tasks, optionally filtered by type (`GET /v1/tasks`).
#[tauri::command]
pub async fn core_list_tasks(
    state: tauri::State<'_, Arc<AppState>>,
    task_type: Option<String>,
) -> Result<Vec<TaskResponse>, String> {
    let tasks = state
        .services
        .task_application
        .list_tasks(task_type.as_deref())
        .await
        .map_err(map_err)?;
    Ok(tasks.into_iter().map(Into::into).collect())
}

/// Get a task by ID (`GET /v1/tasks/{id}`).
#[tauri::command]
pub async fn core_get_task(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResponse, String> {
    Ok(state.services.task_application.get_task(&id).await.map_err(map_err)?.into())
}

/// Get the result payload for a completed task (`GET /v1/tasks/{id}/result`).
#[tauri::command]
pub async fn core_get_task_result(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResultPayload, String> {
    Ok(state.services.task_application.get_task_result(&id).await.map_err(map_err)?.into())
}

/// Cancel a running task (`POST /v1/tasks/{id}/cancel`).
#[tauri::command]
pub async fn core_cancel_task(
    state: tauri::State<'_, Arc<AppState>>,
    id: String,
) -> Result<TaskResponse, String> {
    Ok(state.services.task_application.cancel_task(&id).await.map_err(map_err)?.into())
}

// ── Setup ─────────────────────────────────────────────────────────────────────

/// Return the current environment setup status (`GET /v1/setup/status`).
#[tauri::command]
pub async fn core_setup_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<SetupStatusResponse, String> {
    Ok(state.services.setup.environment_status().await.map_err(map_err)?.into())
}

/// Start downloading FFmpeg (`POST /v1/setup/ffmpeg/download`).
#[tauri::command]
pub async fn core_download_ffmpeg(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<OperationAcceptedResponse, String> {
    let op = state.services.setup.download_ffmpeg().await.map_err(map_err)?;
    Ok(OperationAcceptedResponse { operation_id: op.operation_id })
}

/// Mark setup as complete or reset it (`POST /v1/setup/complete`).
#[tauri::command]
pub async fn core_complete_setup(
    state: tauri::State<'_, Arc<AppState>>,
    req: CompleteSetupRequest,
) -> Result<SetupStatusResponse, String> {
    Ok(state.services.setup.complete_setup(req.into()).await.map_err(map_err)?.into())
}

// ── Backends ──────────────────────────────────────────────────────────────────

/// Get the status of a single backend worker (`GET /v1/backends/status`).
#[tauri::command]
pub async fn core_backend_status(
    state: tauri::State<'_, Arc<AppState>>,
    query: BackendTypeQuery,
) -> Result<BackendStatusResponse, String> {
    let query = validate(query)?;
    Ok(state.services.backend.backend_status(query.into()).await.map_err(map_err)?.into())
}

/// List all registered backend workers (`GET /v1/backends`).
#[tauri::command]
pub async fn core_list_backends(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<BackendListResponse, String> {
    let backends =
        state.services.backend.list_backends().await.map_err(map_err)?;
    Ok(BackendListResponse { backends: backends.into_iter().map(Into::into).collect() })
}

/// Start downloading a backend native library (`POST /v1/backends/download`).
#[tauri::command]
pub async fn core_download_backend_lib(
    state: tauri::State<'_, Arc<AppState>>,
    req: DownloadLibRequest,
) -> Result<OperationAcceptedResponse, String> {
    let req = validate(req)?;
    Ok(state.services.backend.download_lib(req.into()).await.map_err(map_err)?.into())
}

/// Reload a backend with a new native library (`POST /v1/backends/reload`).
#[tauri::command]
pub async fn core_reload_backend_lib(
    state: tauri::State<'_, Arc<AppState>>,
    req: ReloadLibRequest,
) -> Result<BackendStatusResponse, String> {
    let req = validate(req)?;
    let cmd = req.try_into().map_err(map_err)?;
    Ok(state.services.backend.reload_lib(cmd).await.map_err(map_err)?.into())
}

// ── System ────────────────────────────────────────────────────────────────────

/// Return current GPU telemetry (`GET /v1/system/gpu`).
#[tauri::command]
pub async fn core_gpu_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<GpuStatusResponse, String> {
    Ok(state.services.system.gpu_status().await.into())
}

// ── Settings ──────────────────────────────────────────────────────────────────

/// Return the full settings document (`GET /v1/settings`).
#[tauri::command]
pub async fn core_list_settings(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::domain::models::SettingsDocumentView, String> {
    state.services.settings.list_settings().await.map_err(map_err)
}

/// Get a single setting property by PMID (`GET /v1/settings/{pmid}`).
#[tauri::command]
pub async fn core_get_setting(
    state: tauri::State<'_, Arc<AppState>>,
    pmid: String,
) -> Result<crate::domain::models::SettingPropertyView, String> {
    state.services.settings.get_setting(&pmid).await.map_err(map_err)
}

/// Set or unset a setting override (`PUT /v1/settings/{pmid}`).
#[tauri::command]
pub async fn core_update_setting(
    state: tauri::State<'_, Arc<AppState>>,
    pmid: String,
    body: crate::domain::models::UpdateSettingCommand,
) -> Result<crate::domain::models::SettingPropertyView, String> {
    state.services.settings.update_setting(&pmid, body).await.map_err(map_err)
}
