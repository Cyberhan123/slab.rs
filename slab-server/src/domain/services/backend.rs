use std::str::FromStr;

use slab_core::api::Backend;
use strum::IntoEnumIterator;
use tracing::{info, warn};

use crate::context::worker_state::OperationContext;
use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand,
    ReloadBackendLibCommand,
};
use crate::error::ServerError;
use crate::infra::rpc::{self, pb};

pub(crate) type AssetNameResolver = Box<dyn Fn(&str) -> String + Send + 'static>;
type WindowsDownloadSpec = (&'static str, &'static str, &'static str, fn(&str) -> String);

#[derive(Clone)]
pub struct BackendService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl BackendService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self {
            model_state,
            worker_state,
        }
    }

    pub async fn backend_status(
        &self,
        query: BackendStatusQuery,
    ) -> Result<BackendStatusView, ServerError> {
        let backend = Backend::from_str(&query.backend_id).map_err(|_| {
            ServerError::BadRequest(format!("unknown backend_id: {}", query.backend_id))
        })?;
        let canonical_backend = backend.to_string();
        let status = if self.model_state.grpc().has_backend(&canonical_backend) {
            "ready"
        } else {
            "disabled"
        };
        Ok(BackendStatusView {
            backend: canonical_backend,
            status: status.into(),
        })
    }

    pub async fn list_backends(&self) -> Result<Vec<BackendStatusView>, ServerError> {
        let backends = Backend::iter()
            .map(|name| {
                let backend_str = name.to_string();
                let status = if self.model_state.grpc().has_backend(&backend_str) {
                    "ready"
                } else {
                    "disabled"
                };
                BackendStatusView {
                    backend: backend_str,
                    status: status.into(),
                }
            })
            .collect();
        Ok(backends)
    }

    /// Download a backend library release asset.
    ///
    /// The release tag and asset filename are resolved in the following
    /// priority order:
    ///
    /// 1. Values explicitly set in `settings.json` under `setup.backends.*`.
    /// 2. Built-in platform defaults (the previous hard-coded values).
    pub async fn download_lib(
        &self,
        req: DownloadBackendLibCommand,
    ) -> Result<AcceptedOperation, ServerError> {
        if std::env::consts::OS != "windows" {
            return Err(ServerError::BadRequest(
                "download_lib currently supports only Windows hosts".into(),
            ));
        }

        let backend_id = Backend::from_str(&req.backend_id).map_err(|_| {
            ServerError::BadRequest(format!("unknown backend_id: {}", req.backend_id))
        })?;

        let (owner, repo, default_tag, default_asset_fn) = windows_download_spec(backend_id)
            .ok_or_else(|| {
                ServerError::BadRequest(format!("unsupported backend_id: {backend_id}"))
            })?;

        let config = self.model_state.pmid().config();
        let backend_settings = match backend_id {
            Backend::GGMLLlama => config.setup.backends.ggml_llama,
            Backend::GGMLWhisper => config.setup.backends.ggml_whisper,
            Backend::GGMLDiffusion => config.setup.backends.ggml_diffusion,
            Backend::CandleLlama => config.setup.backends.candle_llama,
            Backend::CandleWhisper => config.setup.backends.candle_whisper,
            Backend::CandleDiffusion => config.setup.backends.candle_diffusion,
            Backend::Onnx => config.setup.backends.onnx,
        };
        let tag = backend_settings
            .tag
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_tag.to_owned());
        let asset = backend_settings
            .asset
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_asset_fn(&tag));
        let backends_dir = config.setup.backends.dir.filter(|s| !s.is_empty());

        let target_dir = backends_dir.unwrap_or(req.target_dir);

        let input_data = serde_json::json!({
            "backend_id": req.backend_id,
            "owner": owner,
            "repo": repo,
            "tag": tag,
            "target_dir": target_dir,
            "asset_name": asset,
        })
        .to_string();

        let asset_clone = asset.clone();
        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending("lib_download", None, Some(input_data.clone())),
                move |operation| {
                    run_libfetch_download(
                        operation,
                        input_data,
                        Box::new(move |_| asset_clone.clone()),
                    )
                },
            )
            .await?;

        Ok(AcceptedOperation { operation_id })
    }

    pub async fn reload_lib(
        &self,
        req: ReloadBackendLibCommand,
    ) -> Result<BackendStatusView, ServerError> {
        let backend_id = req.backend_id.clone();

        info!(backend = %backend_id, lib_path = %req.lib_path, "reloading lib");

        let backend = Backend::from_str(&backend_id)
            .map_err(|_| ServerError::BadRequest(format!("unknown backend: {backend_id}")))?;
        let canonical_backend = backend.to_string();
        let channel = self
            .model_state
            .grpc()
            .backend_channel(&canonical_backend)
            .ok_or_else(|| {
                ServerError::BackendNotReady(format!(
                    "{canonical_backend} gRPC endpoint is not configured"
                ))
            })?;
        let grpc_req = pb::ReloadLibraryRequest {
            lib_path: req.lib_path,
            model_path: req.model_path,
            num_workers: req.num_workers,
            context_length: 0,
        };
        let response = rpc::client::reload_library(channel, &canonical_backend, grpc_req)
            .await
            .map_err(|error| {
                ServerError::Internal(format!("grpc reload_library failed: {error}"))
            })?;

        Ok(BackendStatusView {
            backend: response.backend,
            status: response.status,
        })
    }
}

// ── static download specs (fallback defaults) ────────────────────────────────

fn windows_download_spec(backend_id: Backend) -> Option<WindowsDownloadSpec> {
    match backend_id {
        Backend::GGMLLlama => Some(("ggml-org", "llama.cpp", "b8069", |version: &str| {
            format!("llama-{version}-bin-win-cpu-x64.zip")
        })),
        Backend::GGMLWhisper => Some(("ggml-org", "whisper.cpp", "v1.8.3", |_| {
            "whisper-cublas-12.4.0-bin-x64.zip".to_string()
        })),
        Backend::GGMLDiffusion => Some((
            "leejet",
            "stable-diffusion.cpp",
            "master-504-636d3cb",
            |version: &str| format!("stable-diffusion-{version}-bin-win-cpu-x64.zip"),
        )),
        Backend::CandleDiffusion
        | Backend::CandleLlama
        | Backend::CandleWhisper
        | Backend::Onnx => Some(("slab", "slab-buildin", "v0.1.0", |version: &str| {
            format!("slab-buildin-{version}-bin-win-x64.zip")
        })), // no download specs for candle backends or onnx
    }
}

// ── shared task runner (pub(crate) so SetupService can reuse it) ─────────────

pub(crate) async fn run_libfetch_download(
    operation: OperationContext,
    input_data: String,
    default_asset_fn: AssetNameResolver,
) {
    let operation_id = operation.id().to_owned();
    if let Err(error) = operation.mark_running().await {
        warn!(task_id = %operation_id, error = %error, "failed to mark lib download running");
        return;
    }

    let input: serde_json::Value = match serde_json::from_str(&input_data) {
        Ok(value) => value,
        Err(error) => {
            warn!(task_id = %operation_id, error = %error, "invalid stored input_data for download task");
            let message = format!("invalid stored input_data: {error}");
            if let Err(db_error) = operation.mark_failed(&message).await {
                warn!(task_id = %operation_id, error = %db_error, "failed to persist lib download parse error");
            }
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
        if let Err(db_error) = operation
            .mark_failed("owner, repo, and target_dir are required")
            .await
        {
            warn!(task_id = %operation_id, error = %db_error, "failed to persist lib download validation error");
        }
        return;
    }

    let repo_full = format!("{owner}/{repo}");
    let api = slab_libfetch::Api::new()
        .set_install_dir(std::path::Path::new(&target_dir))
        .repo(repo_full);
    let version_api = match tag.as_deref() {
        Some(version) => api.version(version),
        None => api.latest(),
    };

    let asset_resolver: AssetNameResolver = match asset_name {
        Some(name) => Box::new(move |_| name.clone()),
        None => default_asset_fn,
    };

    match version_api.install(asset_resolver).await {
        Ok(path) => {
            let result_json = serde_json::json!({ "path": path }).to_string();
            if let Err(db_error) = operation.mark_succeeded(&result_json).await {
                warn!(task_id = %operation_id, error = %db_error, "failed to persist lib download success");
            }
        }
        Err(error) => {
            let message = error.to_string();
            if let Err(db_error) = operation.mark_failed(&message).await {
                warn!(task_id = %operation_id, error = %db_error, "failed to persist lib download failure");
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn windows_download_spec_llama() {
        let (owner, repo, tag, asset) =
            windows_download_spec(Backend::GGMLLlama).expect("llama preset");
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
