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
    ModelStatus, StoredModelConfig, UnifiedModel, UnifiedModelStatus, UpdateModelCommand,
};
use crate::error::ServerError;
use crate::infra::db::{ModelStore, UnifiedModelRecord};
use crate::infra::model_configs;
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

    pub async fn create_model(&self, req: CreateModelCommand) -> Result<UnifiedModel, ServerError> {
        self.persist_model_definition(req).await
    }

    pub async fn import_model_config(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, ServerError> {
        self.persist_model_definition(req).await
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

        let next = CreateModelCommand {
            id: Some(existing_model.id),
            display_name: req.display_name.unwrap_or(existing_model.display_name),
            provider: req.provider.unwrap_or(existing_model.provider),
            status: Some(req.status.unwrap_or(existing_model.status)),
            spec: req.spec.unwrap_or(existing_model.spec),
            runtime_presets: req.runtime_presets.or(existing_model.runtime_presets),
        };

        self.persist_model_definition(next).await
    }

    pub async fn delete_model(&self, id: &str) -> Result<DeletedModelView, ServerError> {
        let exists = self.model_state.store().get_model(id).await?;
        if exists.is_none() {
            return Err(ServerError::NotFound(format!("model {id} not found")));
        }

        let _ = model_configs::delete_model_config(self.model_config_dir(), id)?;
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

    pub async fn sync_model_configs_from_disk(&self) -> Result<(), ServerError> {
        let config_dir = self.model_config_dir().to_path_buf();
        let paths = model_configs::list_model_config_paths(&config_dir)?;
        if paths.is_empty() {
            info!(path = %config_dir.display(), "no model config files found during startup");
            return Ok(());
        }

        let mut imported = 0usize;
        for path in paths {
            let config = match model_configs::read_model_config(&path) {
                Ok(config) => config,
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "skipping invalid model config file");
                    continue;
                }
            };

            match self.persist_model_definition(config.into()).await {
                Ok(model) => {
                    imported += 1;
                    info!(model_id = %model.id, path = %path.display(), "initialized model from config file");
                }
                Err(error) => {
                    warn!(path = %path.display(), error = %error, "failed to initialize model from config file");
                }
            }
        }

        info!(
            path = %config_dir.display(),
            imported,
            "model config startup sync complete"
        );
        Ok(())
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
        let model_config_dir = self.model_config_dir().to_path_buf();
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

                            let updated_record = match store.get_model(&model_id).await {
                                Ok(Some(record)) => record,
                                Ok(None) => {
                                    let message = format!(
                                        "downloaded file but model {model_id} no longer exists after update"
                                    );
                                    if let Err(db_error) = operation.mark_failed(&message).await {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to persist missing model after download");
                                    }
                                    return;
                                }
                                Err(error) => {
                                    let message = format!(
                                        "downloaded file but failed to reload model {model_id}: {error}"
                                    );
                                    if let Err(db_error) = operation.mark_failed(&message).await {
                                        warn!(task_id = %operation_id, error = %db_error, "failed to persist model reload error after download");
                                    }
                                    return;
                                }
                            };

                            if let Err(error) =
                                sync_model_config_record(&model_config_dir, updated_record)
                            {
                                warn!(task_id = %operation_id, error = %error, "failed to sync downloaded model config file");
                                let message = format!(
                                    "downloaded file but failed to sync model config: {error}"
                                );
                                if let Err(db_error) = operation.mark_failed(&message).await {
                                    warn!(task_id = %operation_id, error = %db_error, "failed to persist config sync failure after download");
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

    fn model_config_dir(&self) -> &std::path::Path {
        self.model_state.config().model_config_dir.as_path()
    }

    async fn persist_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, ServerError> {
        let model = self.build_model_definition(req).await?;
        self.write_model_config(&model)?;

        let record = model_to_record(&model)?;
        self.model_state.store().upsert_model(record).await?;
        Ok(model)
    }

    async fn build_model_definition(
        &self,
        req: CreateModelCommand,
    ) -> Result<UnifiedModel, ServerError> {
        let id = normalize_required_text(
            req.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            "id",
        )?;
        let display_name = normalize_required_text(req.display_name, "display_name")?;
        let provider = normalize_required_text(req.provider, "provider")?;
        let spec = canonicalize_model_spec(&provider, req.spec)?;
        let runtime_presets = canonicalize_runtime_presets(req.runtime_presets);
        let status = req
            .status
            .unwrap_or_else(|| default_status_for_provider(&provider));

        let existing_record = self.model_state.store().get_model(&id).await?;
        let now = Utc::now();
        let created_at = existing_record
            .as_ref()
            .map(|record| record.created_at)
            .unwrap_or(now);

        Ok(UnifiedModel {
            id,
            display_name,
            provider,
            status,
            spec,
            runtime_presets,
            created_at,
            updated_at: now,
        })
    }

    fn write_model_config(&self, model: &UnifiedModel) -> Result<(), ServerError> {
        let config: StoredModelConfig = model.clone().into();
        model_configs::write_model_config(self.model_config_dir(), &config)?;
        Ok(())
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

fn canonicalize_runtime_presets(
    runtime_presets: Option<crate::domain::models::RuntimePresets>,
) -> Option<crate::domain::models::RuntimePresets> {
    runtime_presets.filter(|presets| presets.temperature.is_some() || presets.top_p.is_some())
}

fn default_status_for_provider(provider: &str) -> UnifiedModelStatus {
    if provider.starts_with("cloud.") {
        UnifiedModelStatus::Ready
    } else {
        UnifiedModelStatus::NotDownloaded
    }
}

fn normalize_required_text(value: String, label: &str) -> Result<String, ServerError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ServerError::BadRequest(format!(
            "{label} must not be empty"
        )));
    }
    Ok(trimmed.to_owned())
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

fn model_to_record(model: &UnifiedModel) -> Result<UnifiedModelRecord, ServerError> {
    let spec_json = serde_json::to_string(&model.spec)
        .map_err(|error| ServerError::Internal(format!("failed to serialize spec: {error}")))?;
    let runtime_presets_json = model
        .runtime_presets
        .as_ref()
        .map(|presets| serde_json::to_string(presets))
        .transpose()
        .map_err(|error| {
            ServerError::Internal(format!("failed to serialize runtime_presets: {error}"))
        })?;

    Ok(UnifiedModelRecord {
        id: model.id.clone(),
        display_name: model.display_name.clone(),
        provider: model.provider.clone(),
        status: model.status.as_str().to_owned(),
        spec: spec_json,
        runtime_presets: runtime_presets_json,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn sync_model_config_record(
    config_dir: &std::path::Path,
    record: UnifiedModelRecord,
) -> Result<(), ServerError> {
    let model: UnifiedModel = record
        .try_into()
        .map_err(|error: String| ServerError::Internal(error))?;
    let config: StoredModelConfig = model.into();
    model_configs::write_model_config(config_dir, &config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_model_spec, canonicalize_runtime_presets, normalize_required_text};
    use crate::domain::models::{ModelSpec, RuntimePresets};

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

    #[test]
    fn empty_runtime_presets_are_dropped() {
        let presets = canonicalize_runtime_presets(Some(RuntimePresets {
            temperature: None,
            top_p: None,
        }));

        assert!(presets.is_none());
    }

    #[test]
    fn required_text_fields_are_trimmed() {
        let value = normalize_required_text("  model-id  ".into(), "id").expect("trimmed value");

        assert_eq!(value, "model-id");
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
