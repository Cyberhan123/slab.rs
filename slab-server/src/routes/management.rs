//! Model-management routes.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{body::Body, Json, Router};
use serde::Deserialize;
use tracing::{info, warn};

use crate::db::{TaskRecord, TaskStore};
use crate::error::ServerError;
use crate::models::management::{LoadModelRequest, ModelStatusResponse, ModelTypePath};
use crate::state::AppState;

/// Register model-management routes.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/models",                          get(list_backends))
        .route("/models/available",               get(list_available_models))
        .route("/models/{model_type}/load",        post(load_model))
        .route("/models/{model_type}/status",      get(model_status))
        .route("/models/{model_type}/unload",      post(unload_model))
        .route("/models/{model_type}/switch",      post(switch_model))
        .route("/models/{model_type}/download",    post(download_model))
        .route("/models/{model_type}/download-lib",post(download_lib))
        .route("/models/{model_type}/reload-lib",  post(reload_lib))
        .layer(middleware::from_fn_with_state(
            Arc::new(()) as Arc<()>,
            |req: Request<Body>, next: Next| async move {
                check_management_auth(req, next).await
            },
        ))
}

async fn check_management_auth(req: Request<Body>, next: Next) -> Response {
    let expected = std::env::var("SLAB_MANAGEMENT_TOKEN").ok();
    if let Some(expected_token) = expected {
        let provided = req
            .headers()
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        match provided {
            Some(token) if token == expected_token => {}
            _ => {
                return (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({ "error": "unauthorised" })),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

fn backend_id(model_type: &str) -> Option<&'static str> {
    match model_type {
        "llama"     => Some("ggml.llama"),
        "whisper"   => Some("ggml.whisper"),
        "diffusion" => Some("ggml.diffusion"),
        _           => None,
    }
}

fn validate_path(label: &str, path: &str) -> Result<(), ServerError> {
    if path.is_empty() {
        return Err(ServerError::BadRequest(format!("{label} must not be empty")));
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

// ── Request types for new handlers ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct SwitchModelRequest {
    pub model_path: String,
    pub lib_path: Option<String>,
    #[serde(default = "default_workers")]
    pub num_workers: u32,
}

#[derive(Deserialize)]
pub struct DownloadModelRequest {
    /// HuggingFace repo id, e.g. `"bartowski/Qwen2.5-0.5B-Instruct-GGUF"`.
    pub repo_id: String,
    /// Filename inside the repo to download, e.g. `"Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"`.
    pub filename: String,
    /// Optional directory where the downloaded file will be placed.
    /// If omitted, the hf-hub default cache (`~/.cache/huggingface/hub`) is used.
    pub target_dir: Option<String>,
}

#[derive(Deserialize)]
pub struct DownloadLibRequest {
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub tag: Option<String>,
    pub target_path: String,
    pub asset_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ReloadLibRequest {
    pub lib_path: String,
    pub model_path: String,
    #[serde(default = "default_workers")]
    pub num_workers: u32,
}

fn default_workers() -> u32 { 1 }

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Shared logic for libfetch-backed download tasks (models and libraries).
///
/// Both `download_model` and `download_lib` follow identical patterns:
/// validate input, build an asset-name closure specific to the artifact type,
/// and run `VersionApi::install` in a background task.
async fn run_libfetch_download(
    store: Arc<crate::db::sqlite::SqliteStore>,
    task_manager: Arc<crate::state::TaskManager>,
    tid: String,
    input_data: String,
    default_asset_fn: Box<dyn Fn(&str) -> String + Send + 'static>,
) {
    store.update_task_status(&tid, "running", None, None).await.ok();

    let input: serde_json::Value = match serde_json::from_str(&input_data) {
        Ok(v) => v,
        Err(e) => {
            warn!(task_id = %tid, error = %e, "invalid stored input_data for download task");
            store.update_task_status(&tid, "failed", None, Some(&format!("invalid stored input_data: {e}"))).await.ok();
            task_manager.remove(&tid);
            return;
        }
    };
    let owner       = input["owner"].as_str().unwrap_or("").to_owned();
    let repo        = input["repo"].as_str().unwrap_or("").to_owned();
    let tag         = input["tag"].as_str().map(str::to_owned);
    let target_path = input["target_path"].as_str().unwrap_or("").to_owned();
    let asset_name  = input["asset_name"].as_str().map(str::to_owned);

    if owner.is_empty() || repo.is_empty() {
        store
            .update_task_status(&tid, "failed", None, Some("owner and repo are required"))
            .await
            .ok();
        task_manager.remove(&tid);
        return;
    }

    let target_dir = match std::path::Path::new(&target_path).parent() {
        Some(p) => p.to_owned(),
        None => {
            store
                .update_task_status(
                    &tid,
                    "failed",
                    None,
                    Some("invalid target_path: no parent directory"),
                )
                .await
                .ok();
            task_manager.remove(&tid);
            return;
        }
    };

    let repo_full = format!("{owner}/{repo}");
    let api = slab_libfetch::Api::new()
        .set_install_dir(&target_dir)
        .repo(repo_full);
    let version_api = match tag.as_deref() {
        Some(t) => api.version(t),
        None    => api.latest(),
    };

    let asset_resolver: Box<dyn Fn(&str) -> String + Send> = match asset_name {
        Some(name) => Box::new(move |_| name.clone()),
        None       => default_asset_fn,
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

/// Load (or hot-reload) a model (`POST /api/models/{type}/load`).
#[utoipa::path(
    post,
    path = "/api/models/{model_type}/load",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
    request_body = LoadModelRequest,
    responses(
        (status = 200, description = "Model load initiated",   body = ModelStatusResponse),
        (status = 400, description = "Unknown model type or invalid paths"),
        (status = 401, description = "Unauthorised (management token required)"),
        (status = 500, description = "Backend error"),
    )
)]
pub async fn load_model(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<LoadModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    validate_path("lib_path",   &req.lib_path)?;
    validate_path("model_path", &req.model_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest("num_workers must be at least 1".into()));
    }

    info!(
        backend    = %bid,
        lib_path   = %req.lib_path,
        model_path = %req.model_path,
        workers    = req.num_workers,
        "loading model"
    );

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            "lib_path":    req.lib_path,
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse { backend: bid.to_owned(), status: "loaded".into() }))
}

/// Get status of a model backend (`GET /api/models/{type}/status`).
#[utoipa::path(
    get,
    path = "/api/models/{model_type}/status",
    tag = "management",
    params(("model_type" = String, Path, description = "One of: llama, whisper, diffusion")),
    responses(
        (status = 200, description = "Backend worker is running", body = ModelStatusResponse),
        (status = 400, description = "Unknown model type"),
        (status = 401, description = "Unauthorised (management token required)"),
    )
)]
pub async fn model_status(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    Ok(Json(ModelStatusResponse { backend: bid.to_owned(), status: "ready".into() }))
}

/// Unload the currently loaded model (`POST /api/models/{type}/unload`).
pub async fn unload_model(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    info!(backend = %bid, "unloading model");

    slab_core::api::backend(bid)
        .op("model.unload")
        .input(slab_core::Payload::default())
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse { backend: bid.to_owned(), status: "unloaded".into() }))
}

/// List all registered backends and their status (`GET /api/models`).
pub async fn list_backends(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let backends = ["llama", "whisper", "diffusion"]
        .iter()
        .map(|name| {
            let bid = backend_id(name).unwrap_or("unknown");
            serde_json::json!({ "model_type": name, "backend": bid, "status": "ready" })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "backends": backends })))
}

// ── HuggingFace repo / file listing ──────────────────────────────────────────

/// Query parameters for listing files in a HuggingFace repo.
#[derive(Deserialize)]
pub struct ListAvailableQuery {
    /// HuggingFace repo id, e.g. `"bartowski/Qwen2.5-0.5B-Instruct-GGUF"`.
    pub repo_id: String,
}

/// List the files available in a HuggingFace model repo (`GET /api/models/available?repo_id=...`).
///
/// Uses the `hf-hub` sync API (wrapped in `spawn_blocking`) to fetch repo metadata
/// and returns a list of filenames in the repo.
pub async fn list_available_models(
    State(_state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<ListAvailableQuery>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if q.repo_id.is_empty() {
        return Err(ServerError::BadRequest("repo_id query parameter must not be empty".into()));
    }

    let repo_id = q.repo_id.clone();
    let files: Vec<String> = tokio::task::spawn_blocking(move || {
        use hf_hub::api::sync::Api;
        let api = Api::new().map_err(|e| format!("hf-hub init failed: {e}"))?;
        let repo = api.model(repo_id);
        let info = repo.info().map_err(|e| format!("hf-hub info failed: {e}"))?;
        let names = info.siblings.into_iter().map(|s| s.rfilename).collect();
        Ok::<Vec<String>, String>(names)
    })
    .await
    .map_err(|e| ServerError::Internal(format!("spawn_blocking panicked: {e}")))?
    .map_err(ServerError::Internal)?;

    Ok(Json(serde_json::json!({ "repo_id": q.repo_id, "files": files })))
}

/// Switch the loaded model to a different weights file (`POST /api/models/{type}/switch`).
pub async fn switch_model(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<SwitchModelRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    validate_path("model_path", &req.model_path)?;

    // Resolve lib_path: use request value or fall back to configured lib dir.
    let lib_path = req.lib_path.clone().or_else(|| {
        slab_core::api::lib_dirs().and_then(|dirs| match model_type.as_str() {
            "llama"     => dirs.llama.clone(),
            "whisper"   => dirs.whisper.clone(),
            "diffusion" => dirs.diffusion.clone(),
            _           => None,
        })
    }).ok_or_else(|| {
        ServerError::BadRequest("lib_path required when lib dirs are not configured".into())
    })?;

    validate_path("lib_path", &lib_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest("num_workers must be at least 1".into()));
    }

    info!(backend = %bid, model_path = %req.model_path, "switching model");

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            "lib_path":    lib_path,
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse { backend: bid.to_owned(), status: "loaded".into() }))
}

/// Download a model weight file from HuggingFace (`POST /api/models/{type}/download`).
///
/// Uses the `hf-hub` sync API (wrapped in `spawn_blocking`) to fetch the file
/// from the given HuggingFace repo.  Returns the local cache path where the
/// file was stored.  The download is async; poll `GET /api/tasks/{id}` for status.
pub async fn download_model(
    State(state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<DownloadModelRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if req.repo_id.is_empty() {
        return Err(ServerError::BadRequest("repo_id must not be empty".into()));
    }
    if req.filename.is_empty() {
        return Err(ServerError::BadRequest("filename must not be empty".into()));
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let input_data = serde_json::json!({
        "model_type": model_type,
        "repo_id":    req.repo_id,
        "filename":   req.filename,
        "target_dir": req.target_dir,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "model_download".into(),
            status: "pending".into(),
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

    let join = tokio::spawn(async move {
        store.update_task_status(&tid, "running", None, None).await.ok();

        let input: serde_json::Value = match serde_json::from_str(&input_data) {
            Ok(v) => v,
            Err(e) => {
                warn!(task_id = %tid, error = %e, "invalid stored input_data for model_download task");
                store.update_task_status(&tid, "failed", None, Some(&format!("invalid stored input_data: {e}"))).await.ok();
                task_manager.remove(&tid);
                return;
            }
        };

        let repo_id  = input["repo_id"].as_str().unwrap_or("").to_owned();
        let filename = input["filename"].as_str().unwrap_or("").to_owned();
        let target_dir = input["target_dir"].as_str().map(str::to_owned);

        // Fail early if required fields are missing or empty.
        if repo_id.is_empty() || filename.is_empty() {
            warn!(task_id = %tid, "model_download task is missing repo_id or filename in stored input_data");
            store.update_task_status(&tid, "failed", None, Some("missing repo_id or filename in stored input_data")).await.ok();
            task_manager.remove(&tid);
            return;
        }

        // Validate target_dir to prevent directory traversal.
        if let Some(dir) = &target_dir {
            if let Err(e) = validate_path("target_dir", dir) {
                warn!(task_id = %tid, error = %e, "invalid target_dir in model_download task");
                store.update_task_status(&tid, "failed", None, Some(&e.to_string())).await.ok();
                task_manager.remove(&tid);
                return;
            }
        }

        let result = tokio::task::spawn_blocking(move || {
            use hf_hub::api::sync::{Api, ApiBuilder};
            // If a custom target_dir was provided, configure hf-hub to use it as cache root.
            let api = if let Some(dir) = target_dir {
                ApiBuilder::new()
                    .with_cache_dir(std::path::PathBuf::from(dir))
                    .build()
                    .map_err(|e| format!("hf-hub build failed: {e}"))?
            } else {
                Api::new().map_err(|e| format!("hf-hub init failed: {e}"))?
            };
            let path = api
                .model(repo_id)
                .get(&filename)
                .map_err(|e| format!("hf-hub download failed: {e}"))?;
            Ok::<String, String>(path.to_string_lossy().into_owned())
        })
        .await;

        match result {
            Ok(Ok(local_path)) => {
                let result_json = serde_json::json!({ "local_path": local_path }).to_string();
                store.update_task_status(&tid, "succeeded", Some(&result_json), None).await.ok();
                info!(task_id = %tid, local_path = %local_path, "model download succeeded");
            }
            Ok(Err(e)) => {
                warn!(task_id = %tid, error = %e, "model download failed");
                store.update_task_status(&tid, "failed", None, Some(&e)).await.ok();
            }
            Err(e) => {
                warn!(task_id = %tid, error = %e, "model download task panicked");
                store.update_task_status(&tid, "failed", None, Some(&e.to_string())).await.ok();
            }
        }
        task_manager.remove(&tid);
    });

    state.task_manager.insert(task_id.clone(), join.abort_handle());
    info!(task_id = %task_id, model_type = %model_type, "model download task accepted");
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Download a backend shared library from a GitHub release (`POST /api/models/{type}/download-lib`).
pub async fn download_lib(
    State(state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<DownloadLibRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    validate_path("target_path", &req.target_path)?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let input_data = serde_json::json!({
        "model_type": model_type,
        "owner": req.owner,
        "repo":  req.repo,
        "tag":   req.tag,
        "target_path": req.target_path,
        "asset_name": req.asset_name,
    })
    .to_string();

    state
        .store
        .insert_task(TaskRecord {
            id: task_id.clone(),
            task_type: "lib_download".into(),
            status: "pending".into(),
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
        Box::new(|version: &str| {
            let ext = match std::env::consts::OS {
                "macos"   => "dylib",
                "windows" => "dll",
                _         => "so",
            };
            format!("lib-{version}.{ext}")
        }),
    ));

    state.task_manager.insert(task_id.clone(), join.abort_handle());
    Ok(Json(serde_json::json!({ "task_id": task_id })))
}

/// Reload a backend with a new shared library path (`POST /api/models/{type}/reload-lib`).
pub async fn reload_lib(
    State(_state): State<Arc<AppState>>,
    Path(ModelTypePath { model_type }): Path<ModelTypePath>,
    Json(req): Json<ReloadLibRequest>,
) -> Result<Json<ModelStatusResponse>, ServerError> {
    let bid = backend_id(&model_type)
        .ok_or_else(|| ServerError::BadRequest(format!("unknown model type: {model_type}")))?;

    validate_path("lib_path",   &req.lib_path)?;
    validate_path("model_path", &req.model_path)?;

    if req.num_workers == 0 {
        return Err(ServerError::BadRequest("num_workers must be at least 1".into()));
    }

    info!(backend = %bid, lib_path = %req.lib_path, "reloading lib");

    slab_core::api::backend(bid)
        .op("model.load")
        .input(slab_core::Payload::Json(serde_json::json!({
            "lib_path":    req.lib_path,
            "model_path":  req.model_path,
            "num_workers": req.num_workers,
        })))
        .run_wait()
        .await
        .map_err(ServerError::Runtime)?;

    Ok(Json(ModelStatusResponse { backend: bid.to_owned(), status: "loaded".into() }))
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
    fn backend_id_known() {
        assert_eq!(backend_id("llama"),     Some("ggml.llama"));
        assert_eq!(backend_id("whisper"),   Some("ggml.whisper"));
        assert_eq!(backend_id("diffusion"), Some("ggml.diffusion"));
    }

    #[test]
    fn backend_id_unknown() {
        assert!(backend_id("gpt4").is_none());
    }
}

