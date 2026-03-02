//! Model-management routes.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use slab_core::api::Backend;
use slab_core::api::Event;
use std::str::FromStr;
use tracing::{info, warn};
use utoipa::OpenApi;

use crate::entities::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::schemas::admin::backend::{
    BackendStatusResponse, BackendTypeQuery, DownloadLibRequest, ReloadLibRequest,
    BackendListResponse,
};
use crate::state::AppState;
use strum::IntoEnumIterator;

type AssetNameResolver = Box<dyn Fn(&str) -> String + Send + 'static>;

#[derive(OpenApi)]
#[openapi(
    paths(backend_status, list_backends, download_lib, reload_lib),
    components(schemas(
        DownloadLibRequest,
        ReloadLibRequest,
        BackendTypeQuery,
        BackendStatusResponse,
        BackendListResponse,
    ))
)]
pub struct BackendApi;

/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/backends", get(list_backends))
        .route("/backends/status", get(backend_status))
        .route("/backends/download", post(download_lib))
        .route("/backends/reload", post(reload_lib))
}

fn windows_download_spec(backend_id: Backend) -> Option<(&'static str, &'static str, &'static str, AssetNameResolver)> {
    match backend_id {
        Backend::GGMLLlama => Some((
            "ggml-org",
            "llama.cpp",
            "b8069",
            Box::new(|version: &str| format!("llama-{version}-bin-win-cpu-x64.zip")),
        )),
        Backend::GGMLWhisper => Some((
            "ggml-org",
            "whisper.cpp",
            "v1.8.3",
            Box::new(|_| "whisper-cublas-12.4.0-bin-x64.zip".to_string()),
        )),
        Backend::GGMLDiffusion => Some((
            "leejet",
            "stable-diffusion.cpp",
            "master-504-636d3cb",
            Box::new(|version: &str| format!("stable-diffusion-{version}-bin-win-cpu-x64.zip")),
        )),
        _ => None,
    }
}

fn validate_path(label: &str, path: &str) -> Result<(), ServerError> {
    if path.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "{label} must not be empty"
        )));
    }
    if !std::path::Path::new(path).is_absolute() {
        return Err(ServerError::BadRequest(format!(
            "{label} must be an absolute path (got: {path})"
        )));
    }
    let has_traversal = std::path::Path::new(path)
        .components()
        .any(|c| c == std::path::Component::ParentDir);
    if has_traversal {
        return Err(ServerError::BadRequest(format!(
            "{label} must not contain '..' components"
        )));
    }
    Ok(())
}

/// Shared logic for libfetch-backed download tasks (models and libraries).
///
/// Validates queued task input and runs `VersionApi::install` in a background task.
async fn run_libfetch_download(
    store: Arc<crate::entities::AnyStore>,
    task_manager: Arc<crate::state::TaskManager>,
    tid: String,
    input_data: String,
    default_asset_fn: AssetNameResolver,
) {
    store
        .update_task_status(&tid, "running", None, None)
        .await
        .ok();

    let input: serde_json::Value = match serde_json::from_str(&input_data) {
        Ok(v) => v,
        Err(e) => {
            warn!(task_id = %tid, error = %e, "invalid stored input_data for download task");
            store
                .update_task_status(
                    &tid,
                    "failed",
                    None,
                    Some(&format!("invalid stored input_data: {e}")),
                )
                .await
                .ok();
            task_manager.remove(&tid);
            return;
        }
    };
    let owner = input["owner"].as_str().unwrap_or("").to_owned();
    let repo = input["repo"].as_str().unwrap_or("").to_owned();
    let tag = input["tag"].as_str().map(str::to_owned);
    let target_dir = input["target_dir"]
        .as_str()
        .or_else(|| input["target_path"].as_str())
        .unwrap_or("")
        .to_owned();
    let asset_name = input["asset_name"].as_str().map(str::to_owned);

    if owner.is_empty() || repo.is_empty() || target_dir.is_empty() {
        store
            .update_task_status(
                &tid,
                "failed",
                None,
                Some("owner, repo, and target_dir are required"),
            )
            .await
            .ok();
        task_manager.remove(&tid);
        return;
    }

    let repo_full = format!("{owner}/{repo}");
    let api = slab_libfetch::Api::new()
        .set_install_dir(std::path::Path::new(&target_dir))
        .repo(repo_full);
    let version_api = match tag.as_deref() {
        Some(t) => api.version(t),
        None => api.latest(),
    };

    let asset_resolver: AssetNameResolver = match asset_name {
        Some(name) => Box::new(move |_| name.clone()),
        None => default_asset_fn,
    };

    match version_api.install(asset_resolver).await {
        Ok(path) => {
            let result_json = serde_json::json!({ "path": path }).to_string();
            store
                .update_task_status(&tid, "succeeded", Some(&result_json), None)
                .await
                .ok();
        }
        Err(e) => {
            store
                .update_task_status(&tid, "failed", None, Some(&e.to_string()))
                .await
                .ok();
        }
    }
    task_manager.remove(&tid);
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// Get status of a model backend (`GET /admin/backends/status`).
#[utoipa::path(
    get,
    path = "/admin/backends/status",
    tag = "admin",
  params(BackendTypeQuery),
    responses(
        (status = 200, description = "Backend worker is running", body = BackendStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
pub async fn backend_status(
    State(_state): State<Arc<AppState>>,
    Query(BackendTypeQuery { backend_id }): Query<BackendTypeQuery>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    Ok(Json(BackendStatusResponse {
        backend: backend_id.to_owned(),
        status: "ready".into(),
    }))
}

/// List all registered backends and their status (`GET /admin/backends`).
#[utoipa::path(
    get,
    path = "/admin/backends",
    tag = "admin",
    responses(
        (status = 200, description = "List of all registered backends", body = BackendListResponse),
        (status = 401, description = "Unauthorised (admin token required)"),
    )
)]
pub async fn list_backends(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<BackendListResponse>, ServerError> {
    let backends = Backend::iter()
        .map(|name| {
            let backend_str = name.to_string();
            BackendStatusResponse {
                backend: backend_str.clone(),
                status: "ready".into(), // In a real implementation, you'd check the actual status of each backend.
            }
        })
        .collect::<Vec<BackendStatusResponse>>();
    Ok(Json(BackendListResponse{ backends: backends }))
}

/// Download a backend shared library from a GitHub release (`POST /admin/backends/download`).
#[utoipa::path(
    post,
    path = "/admin/backends/download",
    tag = "admin",
    request_body = DownloadLibRequest,
    responses(
        (status = 200, description = "Download task created", body = serde_json::Value),
        (status = 400, description = "Bad request (invalid path)"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
pub async fn download_lib(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DownloadLibRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    validate_path("target_dir", &req.target_dir)?;

    if std::env::consts::OS != "windows" {
        return Err(ServerError::BadRequest(
            "download_lib currently supports only Windows hosts".into(),
        ));
    }

    let backend_id = Backend::from_str(&req.backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend_id: {}", req.backend_id)))?;

    let (owner, repo, tag, asset_resolver) = windows_download_spec(backend_id)
        .ok_or_else(|| ServerError::BadRequest(format!("unsupported backend_id: {backend_id}")))?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let input_data = serde_json::json!({
        "backend_id": req.backend_id.to_string(),
        "owner": owner,
        "repo": repo,
        "tag": tag,
        "target_dir": req.target_dir,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "lib_download".into(),
            status: "pending".into(),
            model_id: None,
            input_data: Some(input_data.clone()),
            result_data: None,
            error_msg: None,
            core_task_id: None,
            created_at: now,
            updated_at: now,
        })
        .await?;

    let store = Arc::clone(&state.store);
    let task_manager = Arc::clone(&state.task_manager);
    let tid = task_id.clone();

    let join = tokio::spawn(run_libfetch_download(
        store,
        task_manager,
        tid,
        input_data,
        asset_resolver,
    ));

    state
        .task_manager
        .insert(task_id.clone(), join.abort_handle());
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Reload a backend with a new shared library path (`POST /admin/backends/reload`).
#[utoipa::path(
    post,
    path = "/admin/backends/reload",
    tag = "admin",
    request_body = ReloadLibRequest,
    responses(
        (status = 200, description = "Backend reloaded with new library", body = BackendStatusResponse),
        (status = 400, description = "Bad request (invalid path or unknown backend)"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
pub async fn reload_lib(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ReloadLibRequest>,
) -> Result<Json<BackendStatusResponse>, ServerError> {
    let bid = &req.backend_id;

    validate_path("lib_path", &req.lib_path)?;
    validate_path("model_path", &req.model_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest(
            "num_workers must be at least 1".into(),
        ));
    }

    info!(backend = %bid, lib_path = %req.lib_path, "reloading lib");

    let backend = Backend::from_str(bid)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {bid}")))?;

    // Step 1: reload the dynamic library (drops the current model).
    slab_core::api::reload_library(backend, &req.lib_path)
        .await
        .map_err(ServerError::Runtime)?;

    // Step 2: reload the model into the fresh library.
    slab_core::api::backend(backend)
        .op(Event::LoadModel)
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(BackendStatusResponse {
        backend: bid.to_owned(),
        status: "loaded".into(),
    }))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validate_path_empty() {
        assert!(validate_path("lib_path", "").is_err());
    }

    #[test]
    fn validate_path_relative() {
        assert!(validate_path("lib_path", "relative/path.so").is_err());
    }

    #[test]
    fn validate_path_traversal() {
        assert!(validate_path("lib_path", "/safe/../../../etc/passwd").is_err());
    }

    #[test]
    fn validate_path_absolute_ok() {
        assert!(validate_path("lib_path", "/usr/lib/libllama.so").is_ok());
    }

    #[test]
    fn windows_download_spec_llama() {
        let (owner, repo, tag, asset) = windows_download_spec(Backend::GGMLLlama).expect("llama preset");
        assert_eq!(owner, "ggml-org");
        assert_eq!(repo, "llama.cpp");
        assert_eq!(tag, "b8069");
        assert_eq!(asset(tag), "llama-b8069-bin-win-cpu-x64.zip");
    }

    #[test]
    fn windows_download_spec_whisper() {
        let (owner, repo, tag, asset) =
            windows_download_spec(Backend::GGMLWhisper).expect("whisper preset");
        assert_eq!(owner, "ggml-org");
        assert_eq!(repo, "whisper.cpp");
        assert_eq!(tag, "v1.8.3");
        assert_eq!(asset(tag), "whisper-cublas-12.4.0-bin-x64.zip");
    }

    #[test]
    fn windows_download_spec_diffusion() {
        let (owner, repo, tag, asset) =
            windows_download_spec(Backend::GGMLDiffusion).expect("diffusion preset");
        assert_eq!(owner, "leejet");
        assert_eq!(repo, "stable-diffusion.cpp");
        assert_eq!(tag, "master-504-636d3cb");
        assert_eq!(
            asset(tag),
            "stable-diffusion-master-504-636d3cb-bin-win-cpu-x64.zip"
        );
    }
}
