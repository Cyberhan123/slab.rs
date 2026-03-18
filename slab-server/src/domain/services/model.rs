use std::str::FromStr;
use std::sync::Arc;

use chrono::Utc;
use hf_hub::api::sync::{Api, ApiBuilder};
use slab_core::api::Backend;
use tonic::transport::Channel;
use tracing::{info, warn};

use crate::context::{ModelState, SubmitOperation, WorkerState};
use crate::domain::models::{
    AcceptedOperation, AvailableModelsQuery, AvailableModelsView, CreateModelCommand,
    DeletedModelView, DownloadModelCommand, ListModelsFilter, ModelLoadCommand, ModelSpec,
    ModelStatus, UnifiedModel, UnifiedModelStatus, UpdateModelCommand,
};
use crate::error::ServerError;
use crate::infra::db::{ModelStore, UnifiedModelRecord};
use crate::infra::rpc::{self, pb};
use crate::model_auto_unload::LoadedModelSpec;

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;

#[derive(Clone)]
pub struct ModelService {
    model_state: ModelState,
    worker_state: WorkerState,
}

impl ModelService {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self {
            model_state,
            worker_state,
        }
    }

    pub async fn create_model(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, ServerError> {
        let provider = req.provider.trim().to_owned();
        if provider.is_empty() {
            return Err(ServerError::BadRequest("provider must not be empty".into()));
        }
        let spec = canonicalize_model_spec(&provider, req.spec)?;

        let status = req.status.unwrap_or_else(|| {
            if provider.starts_with("cloud.") {
                UnifiedModelStatus::Ready
            } else {
                UnifiedModelStatus::NotDownloaded
            }
        });

        let spec_json = serde_json::to_string(&spec)
            .map_err(|e| ServerError::Internal(format!("failed to serialize spec: {e}")))?;
        let runtime_presets_json = req
            .runtime_presets
            .as_ref()
            .map(|p| serde_json::to_string(p))
            .transpose()
            .map_err(|e| ServerError::Internal(format!("failed to serialize runtime_presets: {e}")))?;

        let now = Utc::now();
        let record = UnifiedModelRecord {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: req.display_name.trim().to_owned(),
            provider,
            status: status.as_str().to_owned(),
            spec: spec_json,
            runtime_presets: runtime_presets_json,
            created_at: now,
            updated_at: now,
        };

        self.model_state
            .store()
            .insert_model(record.clone())
            .await?;

        record
            .try_into()
            .map_err(|e: String| ServerError::Internal(e))
    }

    pub async fn get_model(&self, id: &str) -> Result<UnifiedModel, ServerError> {
        let record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {id} not found")))?;

        record
            .try_into()
            .map_err(|e: String| ServerError::Internal(e))
    }

    pub async fn update_model(
        &self,
        id: &str,
        req: UpdateModelCommand,
    ) -> Result<UnifiedModel, ServerError> {
        let existing_record = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {id} not found")))?;

        let existing_model: UnifiedModel = existing_record
            .try_into()
            .map_err(|e: String| ServerError::Internal(e))?;

        let display_name = req
            .display_name
            .unwrap_or(existing_model.display_name)
            .trim()
            .to_owned();
        let provider = req
            .provider
            .unwrap_or(existing_model.provider)
            .trim()
            .to_owned();
        let status = req.status.unwrap_or(existing_model.status);
        let spec = canonicalize_model_spec(&provider, req.spec.unwrap_or(existing_model.spec))?;
        let runtime_presets = req.runtime_presets.or(existing_model.runtime_presets);

        let spec_json = serde_json::to_string(&spec)
            .map_err(|e| ServerError::Internal(format!("failed to serialize spec: {e}")))?;
        let runtime_presets_json = runtime_presets
            .as_ref()
            .map(|p| serde_json::to_string(p))
            .transpose()
            .map_err(|e| ServerError::Internal(format!("failed to serialize runtime_presets: {e}")))?;

        self.model_state
            .store()
            .update_model(
                id,
                &display_name,
                &provider,
                status.as_str(),
                &spec_json,
                runtime_presets_json.as_deref(),
            )
            .await?;

        let updated = self
            .model_state
            .store()
            .get_model(id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {id} not found after update")))?;

        updated
            .try_into()
            .map_err(|e: String| ServerError::Internal(e))
    }

    pub async fn delete_model(&self, id: &str) -> Result<DeletedModelView, ServerError> {
        let exists = self.model_state.store().get_model(id).await?;
        if exists.is_none() {
            return Err(ServerError::NotFound(format!("model {id} not found")));
        }
        self.model_state.store().delete_model(id).await?;
        Ok(DeletedModelView {
            id: id.to_owned(),
            status: "deleted".to_owned(),
        })
    }

    pub async fn list_models(
        &self,
        _query: ListModelsFilter,
    ) -> Result<Vec<UnifiedModel>, ServerError> {
        let records = self.model_state.store().list_models().await?;
        let models: Vec<UnifiedModel> = records
            .into_iter()
            .filter_map(|record| {
                record
                    .try_into()
                    .map_err(|e: String| {
                        warn!(error = %e, "failed to deserialize model record; skipping");
                    })
                    .ok()
            })
            .collect();
        Ok(models)
    }

    pub async fn load_model(&self, command: ModelLoadCommand) -> Result<ModelStatus, ServerError> {
        self.load_model_command("load_model", "loading model", command)
            .await
    }

    pub async fn unload_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        let backend_id = command.backend_id;
        info!(backend = %backend_id, "unloading model");

        let (canonical_backend, channel) = resolve_backend_channel(&self.model_state, &backend_id)?;
        let response =
            rpc::client::unload_model(channel, &canonical_backend, pb::ModelUnloadRequest {})
                .await
                .map_err(|error| {
                    ServerError::Internal(format!("grpc unload_model failed: {error}"))
                })?;
        self.model_state
            .auto_unload()
            .notify_model_unloaded(&canonical_backend)
            .await;

        Ok(ModelStatus {
            backend: response.backend,
            status: response.status,
        })
    }

    pub async fn list_available_models(
        &self,
        query: AvailableModelsQuery,
    ) -> Result<AvailableModelsView, ServerError> {
        let repo_id = query.repo_id.clone();
        let files: Vec<String> = tokio::task::spawn_blocking(move || {
            let api = Api::new().map_err(|error| format!("hf-hub init failed: {error}"))?;
            let repo = api.model(repo_id);
            let info = repo
                .info()
                .map_err(|error| format!("hf-hub info failed: {error}"))?;
            let names = info
                .siblings
                .into_iter()
                .map(|item| item.rfilename)
                .collect();
            Ok::<Vec<String>, String>(names)
        })
        .await
        .map_err(|error| ServerError::Internal(format!("spawn_blocking panicked: {error}")))?
        .map_err(ServerError::Internal)?;

        Ok(AvailableModelsView {
            repo_id: query.repo_id,
            files,
        })
    }

    pub async fn switch_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        self.load_model_command("switch_model", "switching model", command)
            .await
    }

    pub async fn download_model(
        &self,
        req: DownloadModelCommand,
    ) -> Result<AcceptedOperation, ServerError> {
        let model_id = req.model_id.trim().to_owned();

        let configured_model_cache_dir = self.model_state.pmid().config().runtime.model_cache_dir;
        if let Some(dir) = &configured_model_cache_dir {
            validate_path("model_cache_dir", dir)?;
        }

        let record = self
            .model_state
            .store()
            .get_model(&model_id)
            .await?
            .ok_or_else(|| ServerError::NotFound(format!("model {model_id} not found")))?;

        let model: UnifiedModel = record
            .try_into()
            .map_err(|e: String| ServerError::Internal(e))?;

        // Derive backend from provider. Only local models can be downloaded.
        let backend_id = backend_id_from_provider(&model.provider).ok_or_else(|| {
            ServerError::BadRequest(format!(
                "model provider '{}' does not support download (only providers with prefix \"local.\" support download)",
                model.provider
            ))
        })?;

        let canonical_backend_id = Backend::from_str(&backend_id)
            .map(|b| b.to_string())
            .unwrap_or(backend_id.clone());

        let repo_id = model.spec.repo_id.clone().ok_or_else(|| {
            ServerError::BadRequest(format!(
                "model {model_id} spec is missing repo_id required for download"
            ))
        })?;
        let filename = model.spec.filename.clone().ok_or_else(|| {
            ServerError::BadRequest(format!(
                "model {model_id} spec is missing filename required for download"
            ))
        })?;

        let input_data = serde_json::json!({
            "model_id": model.id,
            "backend_id": canonical_backend_id,
            "repo_id": repo_id,
            "filename": filename,
            "model_cache_dir": configured_model_cache_dir,
        })
        .to_string();

        let store = Arc::clone(self.model_state.store());
        let operation_id = self
            .worker_state
            .submit_operation(
                SubmitOperation::pending(
                    "model_download",
                    Some(model_id.clone()),
                    Some(input_data.clone()),
                ),
                move |operation| async move {
                    let operation_id = operation.id().to_owned();
                    if let Err(error) = operation.mark_running().await {
                        warn!(task_id = %operation_id, error = %error, "failed to mark model download running");
                        return;
                    }

                    let input: serde_json::Value = match serde_json::from_str(&input_data) {
                        Ok(value) => value,
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "invalid stored input_data for model_download task");
                            let message = format!("invalid stored input_data: {error}");
                            if let Err(db_error) = operation.mark_failed(&message).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download parse error");
                            }
                            return;
                        }
                    };

                    let model_id = input["model_id"].as_str().unwrap_or("").to_owned();
                    let repo_id = input["repo_id"].as_str().unwrap_or("").to_owned();
                    let filename = input["filename"].as_str().unwrap_or("").to_owned();
                    let model_cache_dir = input["model_cache_dir"]
                        .as_str()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned);

                    if model_id.is_empty() || repo_id.is_empty() || filename.is_empty() {
                        warn!(task_id = %operation_id, "model_download task is missing model_id, repo_id, or filename");
                        if let Err(db_error) = operation
                            .mark_failed("missing model_id, repo_id, or filename in stored input_data")
                            .await
                        {
                            warn!(task_id = %operation_id, error = %db_error, "failed to persist model download validation error");
                        }
                        return;
                    }

                    let result = tokio::task::spawn_blocking(move || {
                        let api = if let Some(dir) = model_cache_dir {
                            ApiBuilder::new()
                                .with_cache_dir(std::path::PathBuf::from(dir))
                                .build()
                                .map_err(|error| format!("hf-hub build failed: {error}"))?
                        } else {
                            Api::new().map_err(|error| format!("hf-hub init failed: {error}"))?
                        };
                        let path = api
                            .model(repo_id)
                            .get(&filename)
                            .map_err(|error| format!("hf-hub download failed: {error}"))?;
                        Ok::<String, String>(path.to_string_lossy().into_owned())
                    })
                    .await;

                    match result {
                        Ok(Ok(local_path)) => {
                            if let Err(error) = store
                                .update_model_local_path(&model_id, &local_path, "ready")
                                .await
                            {
                                warn!(task_id = %operation_id, error = %error, "failed to persist downloaded model path");
                                let message =
                                    format!("downloaded file but failed to persist path: {error}");
                                if let Err(db_error) = operation.mark_failed(&message).await {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist post-download failure");
                                }
                                return;
                            }

                            let result_json =
                                serde_json::json!({ "local_path": local_path }).to_string();
                            if let Err(db_error) = operation.mark_succeeded(&result_json).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download success");
                            }
                            info!(task_id = %operation_id, local_path = %local_path, "model download succeeded");
                        }
                        Ok(Err(error)) => {
                            warn!(task_id = %operation_id, error = %error, "model download failed");
                            if let Err(db_error) = operation.mark_failed(&error).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download failure");
                            }
                        }
                        Err(error) => {
                            warn!(task_id = %operation_id, error = %error, "model download task panicked");
                            if let Err(db_error) = operation.mark_failed(&error.to_string()).await {
                                warn!(task_id = %operation_id, error = %db_error, "failed to persist model download panic");
                            }
                        }
                    }
                },
            )
            .await?;

        info!(
            task_id = %operation_id,
            backend_id = %backend_id,
            model_id = %model_id,
            "model download task accepted"
        );

        Ok(AcceptedOperation { operation_id })
    }

    async fn load_model_command(
        &self,
        action: &'static str,
        log_message: &'static str,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, ServerError> {
        load_model_with_state(self.model_state.clone(), action, log_message, command).await
    }
}

/// Derive the gRPC backend id from a local provider string.
/// e.g. `"local.ggml.llama"` -> `"ggml.llama"`.
fn backend_id_from_provider(provider: &str) -> Option<String> {
    provider.strip_prefix("local.").map(str::to_owned)
}

fn provider_id_from_provider(provider: &str) -> Option<String> {
    provider
        .strip_prefix("cloud.")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn canonicalize_model_spec(provider: &str, mut spec: ModelSpec) -> Result<ModelSpec, ServerError> {
    spec.provider_id = normalize_optional_text(spec.provider_id);
    spec.remote_model_id = normalize_optional_text(spec.remote_model_id);
    spec.repo_id = normalize_optional_text(spec.repo_id);
    spec.filename = normalize_optional_text(spec.filename);
    spec.local_path = normalize_optional_text(spec.local_path);

    if provider.starts_with("cloud.") {
        if spec.provider_id.is_none() {
            spec.provider_id = provider_id_from_provider(provider);
        }
        if spec.provider_id.is_none() {
            return Err(ServerError::BadRequest(
                "cloud models must set spec.provider_id to a configured chat provider".into(),
            ));
        }
        if spec.remote_model_id.is_none() {
            return Err(ServerError::BadRequest(
                "cloud models must set spec.remote_model_id".into(),
            ));
        }
    }

    Ok(spec)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::canonicalize_model_spec;
    use crate::domain::models::ModelSpec;

    #[test]
    fn cloud_models_require_remote_model_and_provider_reference() {
        let error = canonicalize_model_spec("cloud.openai", ModelSpec::default())
            .expect_err("missing cloud fields");

        assert!(
            error
                .to_string()
                .contains("cloud models must set spec.remote_model_id"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn cloud_models_trim_provider_and_remote_model() {
        let spec = canonicalize_model_spec(
            "cloud.openai",
            ModelSpec {
                provider_id: Some(" openai-main ".into()),
                remote_model_id: Some(" gpt-4.1-mini ".into()),
                ..ModelSpec::default()
            },
        )
        .expect("cloud spec");

        assert_eq!(spec.provider_id.as_deref(), Some("openai-main"));
        assert_eq!(spec.remote_model_id.as_deref(), Some("gpt-4.1-mini"));
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
        .any(|component| component == std::path::Component::ParentDir);
    if has_traversal {
        return Err(ServerError::BadRequest(format!(
            "{label} must not contain '..' components"
        )));
    }
    Ok(())
}

fn validate_existing_model_file(path: &str) -> Result<(), ServerError> {
    let model_path = std::path::Path::new(path);
    if !model_path.exists() {
        return Err(ServerError::BadRequest(format!(
            "model_path does not exist: {path}"
        )));
    }
    if !model_path.is_file() {
        return Err(ServerError::BadRequest(format!(
            "model_path is not a file: {path}"
        )));
    }
    Ok(())
}

fn resolve_backend_channel(
    state: &ModelState,
    backend_id: &str,
) -> Result<(String, Channel), ServerError> {
    let backend = Backend::from_str(backend_id)
        .map_err(|_| ServerError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let canonical_backend = backend.to_string();
    let channel = state
        .grpc()
        .backend_channel(&canonical_backend)
        .ok_or_else(|| {
            ServerError::BackendNotReady(format!(
                "{canonical_backend} gRPC endpoint is not configured"
            ))
        })?;
    Ok((canonical_backend, channel))
}

async fn resolve_model_workers(
    state: &ModelState,
    canonical_backend: &str,
    requested_workers: Option<u32>,
) -> Result<(u32, &'static str), ServerError> {
    if let Some(workers) = requested_workers {
        if workers == 0 {
            return Err(ServerError::BadRequest(
                "num_workers must be at least 1".into(),
            ));
        }
        return Ok((workers, "request"));
    }

    let configured_workers = {
        let config = state.pmid().config();
        match canonical_backend {
            "ggml.llama" => Some(config.runtime.llama.num_workers),
            "ggml.whisper" => Some(config.runtime.whisper.num_workers),
            "ggml.diffusion" => Some(config.runtime.diffusion.num_workers),
            _ => None,
        }
    };
    let Some(workers) = configured_workers else {
        return Ok((DEFAULT_MODEL_NUM_WORKERS, "default"));
    };
    Ok((workers, "settings"))
}

async fn resolve_llama_context_length(
    state: &ModelState,
    canonical_backend: &str,
) -> Result<(u32, &'static str), ServerError> {
    if canonical_backend != "ggml.llama" {
        return Ok((0, "not_applicable"));
    }

    let configured = state.pmid().config().runtime.llama.context_length;
    let Some(context_length) = configured else {
        return Ok((0, "default"));
    };
    Ok((context_length, "settings"))
}

#[derive(Default)]
struct DiffusionContextParams {
    diffusion_model_path: String,
    vae_path: String,
    taesd_path: String,
    lora_model_dir: String,
    clip_l_path: String,
    clip_g_path: String,
    t5xxl_path: String,
    flash_attn: bool,
    keep_vae_on_cpu: bool,
    keep_clip_on_cpu: bool,
    offload_params_to_cpu: bool,
}

async fn resolve_diffusion_context_params(
    state: &ModelState,
    canonical_backend: &str,
) -> Result<Option<DiffusionContextParams>, ServerError> {
    if canonical_backend != "ggml.diffusion" {
        return Ok(None);
    }

    let config = state.pmid().config();
    let paths = config.diffusion.paths;
    let performance = config.diffusion.performance;

    Ok(Some(DiffusionContextParams {
        diffusion_model_path: paths.model.unwrap_or_default(),
        vae_path: paths.vae.unwrap_or_default(),
        taesd_path: paths.taesd.unwrap_or_default(),
        lora_model_dir: paths.lora_model_dir.unwrap_or_default(),
        clip_l_path: paths.clip_l.unwrap_or_default(),
        clip_g_path: paths.clip_g.unwrap_or_default(),
        t5xxl_path: paths.t5xxl.unwrap_or_default(),
        flash_attn: performance.flash_attn,
        keep_vae_on_cpu: performance.keep_vae_on_cpu,
        keep_clip_on_cpu: performance.keep_clip_on_cpu,
        offload_params_to_cpu: performance.offload_params_to_cpu,
    }))
}

fn grpc_status_message(status: &tonic::Status) -> String {
    let message = status.message().trim();
    if !message.is_empty() {
        return message.to_owned();
    }
    status.to_string()
}

fn map_grpc_model_error(action: &str, err: anyhow::Error) -> ServerError {
    let grpc_status = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<tonic::Status>());

    if let Some(status) = grpc_status {
        let detail = grpc_status_message(status);
        return match status.code() {
            tonic::Code::InvalidArgument
            | tonic::Code::FailedPrecondition
            | tonic::Code::ResourceExhausted => ServerError::BadRequest(detail),
            tonic::Code::NotFound => ServerError::NotFound(detail),
            tonic::Code::Unavailable => ServerError::BackendNotReady(detail),
            _ => ServerError::Internal(format!("grpc {action} failed: {err:#}")),
        };
    }

    ServerError::Internal(format!("grpc {action} failed: {err:#}"))
}

async fn load_model_with_state(
    state: ModelState,
    action: &'static str,
    log_message: &'static str,
    command: ModelLoadCommand,
) -> Result<ModelStatus, ServerError> {
    let backend_id = &command.backend_id;

    validate_path("model_path", &command.model_path)?;
    validate_existing_model_file(&command.model_path)?;

    let (canonical_backend, channel) = resolve_backend_channel(&state, backend_id)?;
    let (num_workers, worker_source) =
        resolve_model_workers(&state, &canonical_backend, command.num_workers).await?;
    let (context_length, context_source) =
        resolve_llama_context_length(&state, &canonical_backend).await?;

    info!(
        backend = %backend_id,
        model_path = %command.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "{log_message}"
    );

    let diffusion_ctx = resolve_diffusion_context_params(&state, &canonical_backend)
        .await?
        .unwrap_or_default();

    let grpc_req = pb::ModelLoadRequest {
        model_path: command.model_path.clone(),
        num_workers,
        context_length,
        diffusion_model_path: diffusion_ctx.diffusion_model_path,
        vae_path: diffusion_ctx.vae_path,
        taesd_path: diffusion_ctx.taesd_path,
        lora_model_dir: diffusion_ctx.lora_model_dir,
        clip_l_path: diffusion_ctx.clip_l_path,
        clip_g_path: diffusion_ctx.clip_g_path,
        t5xxl_path: diffusion_ctx.t5xxl_path,
        flash_attn: diffusion_ctx.flash_attn,
        keep_vae_on_cpu: diffusion_ctx.keep_vae_on_cpu,
        keep_clip_on_cpu: diffusion_ctx.keep_clip_on_cpu,
        offload_params_to_cpu: diffusion_ctx.offload_params_to_cpu,
    };
    let response = rpc::client::load_model(channel, &canonical_backend, grpc_req)
        .await
        .map_err(|error| map_grpc_model_error(action, error))?;
    state
        .auto_unload()
        .notify_model_loaded(
            &canonical_backend,
            LoadedModelSpec {
                model_path: command.model_path,
                num_workers,
                context_length,
            },
        )
        .await;

    Ok(ModelStatus {
        backend: response.backend,
        status: response.status,
    })
}
