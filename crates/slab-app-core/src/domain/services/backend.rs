use slab_proto::convert;
use slab_types::RuntimeBackendId;
use tracing::{info, warn};

use crate::context::worker_state::OperationContext;
use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand,
    ReloadBackendLibCommand,
};
use crate::error::AppCoreError;
use crate::infra::rpc;
use crate::runtime_supervisor::RuntimeBackendRuntimeStatus;

pub(crate) type AssetNameResolver = Box<dyn Fn(&str) -> String + Send + 'static>;
type WindowsDownloadSpec = (&'static str, &'static str, &'static str, fn(&str) -> String);

#[derive(Clone)]
pub struct BackendService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl BackendService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self { model_state, worker_state }
    }

    pub async fn backend_status(
        &self,
        query: BackendStatusQuery,
    ) -> Result<BackendStatusView, AppCoreError> {
        let canonical_backend = query.backend_id.to_string();
        Ok(BackendStatusView {
            backend: canonical_backend,
            status: runtime_status_label(
                self.model_state.runtime_status().status(query.backend_id),
            )
            .to_owned(),
        })
    }

    pub async fn list_backends(&self) -> Result<Vec<BackendStatusView>, AppCoreError> {
        let backends = RuntimeBackendId::ALL
            .into_iter()
            .map(|name| BackendStatusView {
                backend: name.to_string(),
                status: runtime_status_label(self.model_state.runtime_status().status(name))
                    .to_owned(),
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
    ) -> Result<AcceptedOperation, AppCoreError> {
        if std::env::consts::OS != "windows" {
            return Err(AppCoreError::BadRequest(
                "download_lib currently supports only Windows hosts".into(),
            ));
        }

        let (owner, repo, default_tag, default_asset_fn) = windows_download_spec(req.backend_id)
            .ok_or_else(|| {
                AppCoreError::BadRequest(format!("unsupported backend_id: {}", req.backend_id))
            })?;

        let config = self.model_state.pmid().config();
        let backend_settings = match req.backend_id {
            RuntimeBackendId::GgmlLlama => config.setup.backends.ggml_llama,
            RuntimeBackendId::GgmlWhisper => config.setup.backends.ggml_whisper,
            RuntimeBackendId::GgmlDiffusion => config.setup.backends.ggml_diffusion,
            RuntimeBackendId::CandleLlama => config.setup.backends.candle_llama,
            RuntimeBackendId::CandleWhisper => config.setup.backends.candle_whisper,
            RuntimeBackendId::CandleDiffusion => config.setup.backends.candle_diffusion,
            RuntimeBackendId::Onnx => config.setup.backends.onnx,
            _ => config.setup.backends.onnx,
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
            "backend_id": req.backend_id.canonical_id(),
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
    ) -> Result<BackendStatusView, AppCoreError> {
        info!(
            backend = %req.backend_id,
            lib_path = %req.spec.lib_path.display(),
            model_path = %req.spec.load.model_path.display(),
            "reloading lib"
        );

        if req.uses_legacy_flattened_load {
            warn!(
                backend = %req.backend_id,
                "legacy flattened reload payload used; prefer {{ lib_path, load }}"
            );
        }

        let canonical_backend = req.backend_id.to_string();
        let channel = self.model_state.grpc().backend_channel(req.backend_id).ok_or_else(|| {
            AppCoreError::BackendNotReady(format!(
                "{canonical_backend} gRPC endpoint is not configured"
            ))
        })?;
        let grpc_req = convert::encode_reload_library_request(&req.spec);
        let response = rpc::client::reload_library(channel, req.backend_id, grpc_req)
            .await
            .map_err(|error| {
                if let Some(detail) = rpc::client::transient_runtime_detail(&error) {
                    AppCoreError::BackendNotReady(detail)
                } else {
                    AppCoreError::Internal(format!("grpc reload_library failed: {error}"))
                }
            })?;

        let status = convert::decode_model_status_response(&response).map_err(|error| {
            AppCoreError::Internal(format!("invalid model status response from runtime: {error}"))
        })?;

        Ok(BackendStatusView { backend: status.backend.to_string(), status: status.status })
    }
}

fn runtime_status_label(status: RuntimeBackendRuntimeStatus) -> &'static str {
    status.as_str()
}

#[cfg(test)]
mod tests {
    use super::runtime_status_label;
    use crate::runtime_supervisor::RuntimeBackendRuntimeStatus;

    #[test]
    fn runtime_status_labels_match_backend_api_surface() {
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Ready), "ready");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Restarting), "restarting");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Unavailable), "unavailable");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Disabled), "disabled");
    }
}

// ── static download specs (fallback defaults) ────────────────────────────────

fn windows_download_spec(backend_id: RuntimeBackendId) -> Option<WindowsDownloadSpec> {
    match backend_id {
        RuntimeBackendId::GgmlLlama => Some(("ggml-org", "llama.cpp", "b8069", |version: &str| {
            format!("llama-{version}-bin-win-cpu-x64.zip")
        })),
        RuntimeBackendId::GgmlWhisper => Some(("ggml-org", "whisper.cpp", "v1.8.3", |_| {
            "whisper-cublas-12.4.0-bin-x64.zip".to_string()
        })),
        RuntimeBackendId::GgmlDiffusion => {
            Some(("leejet", "stable-diffusion.cpp", "master-504-636d3cb", |version: &str| {
                format!("stable-diffusion-{version}-bin-win-cpu-x64.zip")
            }))
        }
        RuntimeBackendId::CandleDiffusion
        | RuntimeBackendId::CandleLlama
        | RuntimeBackendId::CandleWhisper
        | RuntimeBackendId::Onnx => Some(("slab", "slab-buildin", "v0.1.0", |version: &str| {
            format!("slab-buildin-{version}-bin-win-x64.zip")
        })), // no download specs for candle backends or onnx
        _ => None,
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
        if let Err(db_error) =
            operation.mark_failed("owner, repo, and target_dir are required").await
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
            windows_download_spec(RuntimeBackendId::GgmlLlama).expect("llama preset");
        assert_eq!(owner, "ggml-org");
        assert_eq!(repo, "llama.cpp");
        assert_eq!(tag, "b8069");
        assert_eq!(asset(tag), "llama-b8069-bin-win-cpu-x64.zip");
    }

    #[test]
    fn windows_download_spec_whisper() {
        let (owner, repo, tag, asset) =
            windows_download_spec(RuntimeBackendId::GgmlWhisper).expect("whisper preset");
        assert_eq!(owner, "ggml-org");
        assert_eq!(repo, "whisper.cpp");
        assert_eq!(tag, "v1.8.3");
        assert_eq!(asset(tag), "whisper-cublas-12.4.0-bin-x64.zip");
    }

    #[test]
    fn windows_download_spec_diffusion() {
        let (owner, repo, tag, asset) =
            windows_download_spec(RuntimeBackendId::GgmlDiffusion).expect("diffusion preset");
        assert_eq!(owner, "leejet");
        assert_eq!(repo, "stable-diffusion.cpp");
        assert_eq!(tag, "master-504-636d3cb");
        assert_eq!(asset(tag), "stable-diffusion-master-504-636d3cb-bin-win-cpu-x64.zip");
    }
}
