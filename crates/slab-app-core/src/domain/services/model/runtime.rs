use std::path::PathBuf;

use slab_proto::convert;
use slab_types::load_config::{
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig,
};
use slab_types::runtime::DiffusionLoadOptions;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tonic::transport::Channel;
use tracing::info;

use crate::context::ModelState;
use crate::domain::models::{ModelLoadCommand, ModelStatus, UnifiedModel, UnifiedModelKind};
use crate::error::AppCoreError;
use crate::infra::db::{ModelConfigStateStore, ModelStore};
use crate::infra::model_packs;
use crate::infra::rpc::{self, pb};
use crate::model_auto_unload::{ModelReplayPlan, build_model_load_request};

use super::{ModelService, catalog, pack};

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;

#[derive(Debug, Clone)]
struct ResolvedModelLoadTarget {
    backend_id: RuntimeBackendId,
    model_path: String,
    model_id: Option<String>,
    pack_load_defaults: Option<slab_model_pack::ModelPackLoadDefaults>,
}

impl ModelService {
    pub async fn load_model(&self, command: ModelLoadCommand) -> Result<ModelStatus, AppCoreError> {
        self.load_model_command("load_model", "loading model", command).await
    }

    pub async fn unload_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        let backend_id = resolve_unload_backend(&self.model_state, &command).await?;
        info!(backend = %backend_id, model_id = ?command.model_id, "unloading model");

        let (_, channel) = resolve_backend_channel(&self.model_state, backend_id)?;
        let response = rpc::client::unload_model(channel, backend_id, pb::ModelUnloadRequest {})
            .await
            .map_err(|error| map_grpc_model_error("unload_model", error))?;
        self.model_state.auto_unload().notify_model_unloaded(backend_id).await;

        decode_model_status(response)
    }

    pub async fn switch_model(
        &self,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        self.load_model_command("switch_model", "switching model", command).await
    }

    async fn load_model_command(
        &self,
        action: &'static str,
        log_message: &'static str,
        command: ModelLoadCommand,
    ) -> Result<ModelStatus, AppCoreError> {
        load_model_with_state(self.model_state.clone(), action, log_message, command).await
    }
}

pub(super) fn validate_and_normalize_model_workers(
    backend_id: RuntimeBackendId,
    workers: u32,
    source: &'static str,
) -> Result<(u32, &'static str), AppCoreError> {
    if workers == 0 {
        return Err(AppCoreError::BadRequest("num_workers must be at least 1".into()));
    }

    if backend_id == RuntimeBackendId::GgmlDiffusion && workers > 1 {
        tracing::warn!(
            backend = %backend_id,
            requested_workers = workers,
            worker_source = source,
            "ggml.diffusion currently supports only one effective worker; clamping num_workers to 1 to avoid inconsistent per-worker model state"
        );
        return Ok((1, source));
    }

    Ok((workers, source))
}

fn resolve_backend_channel(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<(RuntimeBackendId, Channel), AppCoreError> {
    let canonical_backend = backend_id.to_string();
    let channel = state.grpc().backend_channel(backend_id).ok_or_else(|| {
        AppCoreError::BackendNotReady(format!(
            "{canonical_backend} gRPC endpoint is not configured"
        ))
    })?;
    Ok((backend_id, channel))
}

pub(super) async fn resolve_model_workers(
    state: &ModelState,
    backend_id: RuntimeBackendId,
    requested_workers: Option<u32>,
) -> Result<(u32, &'static str), AppCoreError> {
    if let Some(workers) = requested_workers {
        return validate_and_normalize_model_workers(backend_id, workers, "request");
    }

    let configured_workers = {
        let config = state.pmid().config();
        match backend_id {
            RuntimeBackendId::GgmlLlama => Some(config.runtime.llama.num_workers),
            RuntimeBackendId::GgmlWhisper => Some(config.runtime.whisper.num_workers),
            RuntimeBackendId::GgmlDiffusion => Some(config.runtime.diffusion.num_workers),
            _ => None,
        }
    };
    let Some(workers) = configured_workers else {
        return validate_and_normalize_model_workers(
            backend_id,
            DEFAULT_MODEL_NUM_WORKERS,
            "default",
        );
    };
    validate_and_normalize_model_workers(backend_id, workers, "settings")
}

pub(super) async fn resolve_llama_context_length(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<(u32, &'static str), AppCoreError> {
    if backend_id != RuntimeBackendId::GgmlLlama {
        return Ok((0, "not_applicable"));
    }

    let configured = state.pmid().config().runtime.llama.context_length;
    let Some(context_length) = configured else {
        return Ok((0, "default"));
    };
    Ok((context_length, "settings"))
}

pub(super) async fn resolve_diffusion_context_params(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<Option<DiffusionLoadOptions>, AppCoreError> {
    if backend_id != RuntimeBackendId::GgmlDiffusion {
        return Ok(None);
    }

    let config = state.pmid().config();
    let paths = config.diffusion.paths;
    let performance = config.diffusion.performance;

    Ok(Some(DiffusionLoadOptions {
        diffusion_model_path: paths.model.map(PathBuf::from),
        vae_path: paths.vae.map(PathBuf::from),
        taesd_path: paths.taesd.map(PathBuf::from),
        lora_model_dir: paths.lora_model_dir.map(PathBuf::from),
        clip_l_path: paths.clip_l.map(PathBuf::from),
        clip_g_path: paths.clip_g.map(PathBuf::from),
        t5xxl_path: paths.t5xxl.map(PathBuf::from),
        flash_attn: performance.flash_attn,
        vae_device: performance.vae_device,
        clip_device: performance.clip_device,
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

pub(super) fn decode_model_status(
    response: pb::ModelStatusResponse,
) -> Result<ModelStatus, AppCoreError> {
    let status = convert::decode_model_status_response(&response).map_err(|error| {
        AppCoreError::Internal(format!("invalid model status response from runtime: {error}"))
    })?;

    Ok(ModelStatus { backend: status.backend.to_string(), status: status.status })
}

pub(super) fn map_grpc_model_error(action: &str, err: anyhow::Error) -> AppCoreError {
    if let Some(detail) = rpc::client::transient_runtime_detail(&err) {
        return AppCoreError::BackendNotReady(detail);
    }

    let grpc_status = err.chain().find_map(|cause| cause.downcast_ref::<tonic::Status>());

    if let Some(status) = grpc_status {
        let detail = grpc_status_message(status);
        return match status.code() {
            tonic::Code::InvalidArgument
            | tonic::Code::FailedPrecondition
            | tonic::Code::ResourceExhausted => AppCoreError::BadRequest(detail),
            tonic::Code::NotFound => AppCoreError::NotFound(detail),
            tonic::Code::Unavailable => AppCoreError::BackendNotReady(detail),
            _ => AppCoreError::Internal(format!("grpc {action} failed: {err:#}")),
        };
    }

    AppCoreError::Internal(format!("grpc {action} failed: {err:#}"))
}

async fn load_model_with_state(
    state: ModelState,
    action: &'static str,
    log_message: &'static str,
    command: ModelLoadCommand,
) -> Result<ModelStatus, AppCoreError> {
    let resolved_target = resolve_model_load_target(&state, &command).await?;

    catalog::validate_path("model_path", &resolved_target.model_path)?;
    catalog::validate_existing_model_file(&resolved_target.model_path)?;

    let (_, channel) = resolve_backend_channel(&state, resolved_target.backend_id)?;
    let (num_workers, worker_source) = if let Some(workers) = command.num_workers {
        validate_and_normalize_model_workers(resolved_target.backend_id, workers, "request")?
    } else if let Some(workers) =
        resolved_target.pack_load_defaults.as_ref().and_then(|defaults| defaults.num_workers)
    {
        validate_and_normalize_model_workers(resolved_target.backend_id, workers, "model_pack")?
    } else {
        resolve_model_workers(&state, resolved_target.backend_id, None).await?
    };
    let (context_length, context_source) =
        resolve_llama_context_length(&state, resolved_target.backend_id).await?;
    let diffusion = if let Some(defaults) = resolved_target.pack_load_defaults.as_ref() {
        model_packs::merge_diffusion_load_defaults(
            defaults.diffusion.clone(),
            resolve_diffusion_context_params(&state, resolved_target.backend_id).await?,
        )
    } else {
        resolve_diffusion_context_params(&state, resolved_target.backend_id).await?
    };

    info!(
        backend = %resolved_target.backend_id,
        model_id = ?resolved_target.model_id,
        model_path = %resolved_target.model_path,
        workers = num_workers,
        worker_source = worker_source,
        context_length = context_length,
        context_source = context_source,
        "{log_message}"
    );

    let load_spec = build_backend_load_spec(
        resolved_target.backend_id,
        &resolved_target.model_path,
        num_workers,
        context_length,
        resolved_target
            .pack_load_defaults
            .as_ref()
            .and_then(|defaults| defaults.chat_template.clone()),
        diffusion,
    )?;
    let grpc_req = build_model_load_request(&load_spec);
    let response = rpc::client::load_model(channel, resolved_target.backend_id, grpc_req)
        .await
        .map_err(|error| map_grpc_model_error(action, error))?;
    state
        .auto_unload()
        .notify_model_loaded(ModelReplayPlan {
            backend_id: resolved_target.backend_id,
            model_id: resolved_target.model_id,
            load_spec,
        })
        .await;

    decode_model_status(response)
}

fn build_backend_load_spec(
    backend_id: RuntimeBackendId,
    model_path: &str,
    num_workers: u32,
    context_length: u32,
    chat_template: Option<String>,
    diffusion: Option<DiffusionLoadOptions>,
) -> Result<RuntimeBackendLoadSpec, AppCoreError> {
    let model_path = PathBuf::from(model_path);

    match backend_id {
        RuntimeBackendId::GgmlLlama => Ok(RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
            model_path,
            num_workers: usize::try_from(num_workers).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to convert num_workers into usize for ggml.llama: {error}"
                ))
            })?,
            context_length: (context_length > 0).then_some(context_length),
            chat_template,
        })),
        RuntimeBackendId::GgmlWhisper => {
            Ok(RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig { model_path }))
        }
        RuntimeBackendId::GgmlDiffusion => {
            let diffusion = diffusion.unwrap_or_default();
            Ok(RuntimeBackendLoadSpec::GgmlDiffusion(Box::new(GgmlDiffusionLoadConfig {
                model_path,
                diffusion_model_path: diffusion.diffusion_model_path,
                vae_path: diffusion.vae_path,
                taesd_path: diffusion.taesd_path,
                clip_l_path: diffusion.clip_l_path,
                clip_g_path: diffusion.clip_g_path,
                t5xxl_path: diffusion.t5xxl_path,
                clip_vision_path: None,
                control_net_path: None,
                flash_attn: diffusion.flash_attn,
                vae_device: (!diffusion.vae_device.is_empty()).then_some(diffusion.vae_device),
                clip_device: (!diffusion.clip_device.is_empty()).then_some(diffusion.clip_device),
                offload_params_to_cpu: diffusion.offload_params_to_cpu,
                enable_mmap: false,
                n_threads: None,
            })))
        }
        other => Err(AppCoreError::Internal(format!(
            "unsupported backend for managed model load spec assembly: {}",
            other.canonical_id()
        ))),
    }
}

async fn resolve_model_load_target(
    state: &ModelState,
    command: &ModelLoadCommand,
) -> Result<ResolvedModelLoadTarget, AppCoreError> {
    if let Some(model_id) =
        command.model_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        let model = resolve_local_catalog_model(state, model_id).await?;
        let backend_id = resolve_local_backend_from_model(&model)?;
        let model_path = resolve_local_model_path(&model)?;
        if model_packs::is_model_pack_path(&model_path) {
            let pack_target = build_selected_model_pack_load_target(
                state,
                &model.id,
                std::path::Path::new(&model_path),
            )
            .await?;
            if pack_target.backend_id != backend_id {
                return Err(AppCoreError::BadRequest(format!(
                    "model '{}' pack backend '{}' does not match catalog backend '{}'",
                    model.id, pack_target.backend_id, backend_id
                )));
            }
            return Ok(ResolvedModelLoadTarget {
                backend_id,
                model_path: pack_target.model_path,
                model_id: Some(model.id),
                pack_load_defaults: Some(pack_target.load_defaults),
            });
        }
        return Ok(ResolvedModelLoadTarget {
            backend_id,
            model_path,
            model_id: Some(model.id),
            pack_load_defaults: None,
        });
    }

    let backend_id = command
        .backend_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("backend_id is required when model_id is not provided".into())
        })?;
    let model_path = command
        .model_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("model_path is required when model_id is not provided".into())
        })?
        .to_owned();
    let backend_id: RuntimeBackendId = backend_id
        .parse()
        .map_err(|_| AppCoreError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let (backend_id, _) = resolve_backend_channel(state, backend_id)?;

    if model_packs::is_model_pack_path(&model_path) {
        let pack_target =
            model_packs::build_model_pack_load_target(std::path::Path::new(&model_path))?;
        if pack_target.backend_id != backend_id {
            return Err(AppCoreError::BadRequest(format!(
                "model pack backend '{}' does not match requested backend '{}'",
                pack_target.backend_id, backend_id
            )));
        }
        return Ok(ResolvedModelLoadTarget {
            backend_id,
            model_path: pack_target.model_path,
            model_id: None,
            pack_load_defaults: Some(pack_target.load_defaults),
        });
    }

    Ok(ResolvedModelLoadTarget { backend_id, model_path, model_id: None, pack_load_defaults: None })
}

async fn build_selected_model_pack_load_target(
    state: &ModelState,
    model_id: &str,
    pack_path: &std::path::Path,
) -> Result<model_packs::ModelPackLoadTarget, AppCoreError> {
    let pack = model_packs::open_model_pack(pack_path)?;
    let resolved = pack.resolve().map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to resolve model pack '{}': {error}",
            pack_path.display()
        ))
    })?;
    let persisted = model_packs::read_persisted_model_config_from_pack(pack_path)?;
    let state_record = state.store().get_model_config_state(model_id).await?;
    let legacy_selection = persisted
        .as_ref()
        .and_then(|config| config.pack_selection.clone())
        .map(pack::normalize_model_pack_selection);
    let explicit_selection = if let Some(record) = state_record.as_ref() {
        crate::domain::models::ModelPackSelection {
            preset_id: catalog::normalize_optional_text(record.selected_preset_id.clone()),
            variant_id: catalog::normalize_optional_text(record.selected_variant_id.clone()),
        }
    } else {
        legacy_selection.clone().unwrap_or_default()
    };
    let (effective_selection, selected_preset, _) =
        pack::resolve_effective_model_pack_selection(&resolved, &explicit_selection)?;

    if state_record.is_none()
        && let Some(record) = pack::selection_state_record_for_storage(
            model_id,
            &resolved,
            &legacy_selection.unwrap_or_default(),
            &effective_selection,
        )
    {
        let _ = state.store().upsert_model_config_state(record).await;
    }

    let mut bridge = resolved.compile_runtime_bridge(&selected_preset).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to compile selected pack preset for load target: {error}"
        ))
    })?;
    pack::apply_materialized_source_to_bridge(&mut bridge, persisted.as_ref());
    let preset_id = effective_selection
        .preset_id
        .clone()
        .unwrap_or_else(|| selected_preset.document.id.clone());
    let load_spec = bridge.runtime_load_spec(&preset_id).map_err(|error| match error {
        slab_model_pack::ModelPackError::NonMaterializedSource { .. } => AppCoreError::BadRequest(
            format!(
                "model pack '{}' points to a remote source and must be downloaded from the model catalog before loading",
                pack_path.display()
            ),
        ),
        other => AppCoreError::BadRequest(format!(
            "failed to build selected model pack load target '{}': {other}",
            pack_path.display()
        )),
    })?;

    Ok(model_packs::ModelPackLoadTarget {
        backend_id: bridge.backend,
        model_path: load_spec.model_path.to_string_lossy().into_owned(),
        load_defaults: bridge.load_defaults,
    })
}

async fn resolve_unload_backend(
    state: &ModelState,
    command: &ModelLoadCommand,
) -> Result<RuntimeBackendId, AppCoreError> {
    if let Some(model_id) =
        command.model_id.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        let model = resolve_local_catalog_model(state, model_id).await?;
        let backend_id = resolve_local_backend_from_model(&model)?;
        let (backend_id, _) = resolve_backend_channel(state, backend_id)?;
        return Ok(backend_id);
    }

    let backend_id = command
        .backend_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppCoreError::BadRequest("backend_id is required when model_id is not provided".into())
        })?;
    let backend_id: RuntimeBackendId = backend_id
        .parse()
        .map_err(|_| AppCoreError::BadRequest(format!("unknown backend: {backend_id}")))?;
    let (backend_id, _) = resolve_backend_channel(state, backend_id)?;
    Ok(backend_id)
}

async fn resolve_local_catalog_model(
    state: &ModelState,
    model_id: &str,
) -> Result<UnifiedModel, AppCoreError> {
    let record = state
        .store()
        .get_model(model_id)
        .await?
        .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;
    resolve_local_backend_from_model(&model)?;
    Ok(model)
}

pub(super) fn resolve_local_backend_from_model(
    model: &UnifiedModel,
) -> Result<RuntimeBackendId, AppCoreError> {
    if model.kind != UnifiedModelKind::Local {
        return Err(AppCoreError::BadRequest(format!(
            "model '{}' is '{}' and cannot be managed with local runtime lifecycle endpoints",
            model.id,
            model.kind.as_str()
        )));
    }

    let backend_id = model.backend_id.ok_or_else(|| {
        AppCoreError::BadRequest(format!("model '{}' is local but is missing backend_id", model.id))
    })?;

    Ok(backend_id.into())
}

fn resolve_local_model_path(model: &UnifiedModel) -> Result<String, AppCoreError> {
    model
        .spec
        .local_path
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            AppCoreError::BadRequest(format!(
                "model '{}' has no local_path yet; download it before loading",
                model.id
            ))
        })
}
