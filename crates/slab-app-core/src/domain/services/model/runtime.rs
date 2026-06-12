use std::path::{Path, PathBuf};

use slab_types::load_config::{
    CandleDiffusionLoadConfig, CandleLlamaLoadConfig, CandleWhisperLoadConfig,
    GgmlDiffusionLoadConfig, GgmlLlamaLoadConfig, GgmlWhisperLoadConfig,
};
use slab_types::runtime::DiffusionLoadOptions;
use slab_types::{RuntimeBackendId, RuntimeBackendLoadSpec};
use tracing::info;

use crate::context::{ModelState, WorkerState};
use crate::domain::models::{ModelLoadCommand, ModelStatus, UnifiedModel, UnifiedModelKind};
use crate::domain::ports::RuntimeBackendStatus;
use crate::error::AppCoreError;
use crate::infra::db::{ModelConfigStateStore, ModelStore};
use crate::infra::model_packs;
use crate::model_auto_unload::ModelReplayPlan;

use super::{ModelService, catalog, pack};

const DEFAULT_MODEL_NUM_WORKERS: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct LocalLlamaPromptProfile {
    pub(crate) backend_id: RuntimeBackendId,
    pub(crate) chat_template_source: Option<String>,
    pub(crate) default_gbnf: Option<String>,
}

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

        self.model_state.auto_unload().ensure_idle_for_manual_unload(backend_id).await?;
        let response = self.model_state.runtime().unload_model(backend_id).await?;
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

pub(crate) async fn resolve_local_chat_prompt_profile(
    state: &ModelState,
    model_id: &str,
) -> Result<LocalLlamaPromptProfile, AppCoreError> {
    let model = resolve_local_catalog_model(state, model_id).await?;
    let backend_id = resolve_local_backend_from_model(&model)?;
    if !matches!(backend_id, RuntimeBackendId::GgmlLlama | RuntimeBackendId::CandleLlama) {
        return Err(AppCoreError::BadRequest(format!(
            "model '{model_id}' uses backend '{}' and does not support local chat prompt rendering",
            backend_id.canonical_id()
        )));
    }

    let Some(model_path) = model.spec.local_path.as_deref() else {
        return Ok(LocalLlamaPromptProfile {
            backend_id,
            chat_template_source: None,
            default_gbnf: None,
        });
    };
    if let Some(pack_target) =
        build_catalog_model_pack_load_target(state, &model, model_path).await?
    {
        return Ok(LocalLlamaPromptProfile {
            backend_id,
            chat_template_source: pack_target.load_defaults.chat_template_source,
            default_gbnf: pack_target.load_defaults.gbnf_source,
        });
    }

    Ok(LocalLlamaPromptProfile { backend_id, chat_template_source: None, default_gbnf: None })
}

pub(crate) async fn resolve_worker_model_backend_or_default(
    state: &WorkerState,
    model_id: Option<&str>,
    default_backend: RuntimeBackendId,
) -> Result<RuntimeBackendId, AppCoreError> {
    let Some(model_id) = model_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(default_backend);
    };
    let record = state
        .store()
        .get_model(model_id)
        .await?
        .ok_or_else(|| AppCoreError::NotFound(format!("model {model_id} not found")))?;
    let model: UnifiedModel =
        record.try_into().map_err(|error: String| AppCoreError::Internal(error))?;

    resolve_local_backend_from_model(&model)
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

fn ensure_runtime_backend_available(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<(), AppCoreError> {
    let canonical_backend = backend_id.to_string();
    if !state.runtime().backend_available(backend_id) {
        return Err(AppCoreError::BackendNotReady(format!(
            "{canonical_backend} gRPC endpoint is not configured"
        )));
    }
    Ok(())
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

fn resolve_backend_flash_attn(state: &ModelState, backend_id: RuntimeBackendId) -> bool {
    let config = state.pmid().config();
    match backend_id {
        RuntimeBackendId::GgmlLlama => config.runtime.llama.flash_attn,
        RuntimeBackendId::GgmlWhisper => config.runtime.whisper.flash_attn,
        RuntimeBackendId::GgmlDiffusion => config.diffusion.performance.flash_attn,
        _ => true,
    }
}

pub(super) async fn resolve_diffusion_context_params(
    state: &ModelState,
    backend_id: RuntimeBackendId,
) -> Result<Option<DiffusionLoadOptions>, AppCoreError> {
    if !matches!(backend_id, RuntimeBackendId::GgmlDiffusion | RuntimeBackendId::CandleDiffusion) {
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

pub(super) fn decode_model_status(
    response: RuntimeBackendStatus,
) -> Result<ModelStatus, AppCoreError> {
    Ok(ModelStatus {
        backend: response.backend.to_string(),
        status: response.status,
        context_length: response.context_length,
        training_context_length: response.training_context_length,
    })
}

async fn load_model_with_state(
    state: ModelState,
    _action: &'static str,
    log_message: &'static str,
    command: ModelLoadCommand,
) -> Result<ModelStatus, AppCoreError> {
    let resolved_target = resolve_model_load_target(&state, &command).await?;

    catalog::validate_path("model_path", &resolved_target.model_path)?;
    catalog::validate_existing_model_file(&resolved_target.model_path)?;

    ensure_runtime_backend_available(&state, resolved_target.backend_id)?;
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
    let flash_attn = resolve_backend_flash_attn(&state, resolved_target.backend_id);
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
        flash_attn = flash_attn,
        "{log_message}"
    );

    let load_spec = build_backend_load_spec(
        resolved_target.backend_id,
        &resolved_target.model_path,
        BackendLoadSpecOptions {
            num_workers,
            context_length,
            chat_template: resolved_target
                .pack_load_defaults
                .as_ref()
                .and_then(|defaults| defaults.chat_template_source.clone()),
            gbnf: resolved_target
                .pack_load_defaults
                .as_ref()
                .and_then(|defaults| defaults.gbnf_source.clone()),
            flash_attn,
            diffusion,
        },
    )?;
    let response = state.auto_unload().load_model_with_pressure_control(&load_spec).await?;
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

struct BackendLoadSpecOptions {
    num_workers: u32,
    context_length: u32,
    chat_template: Option<String>,
    gbnf: Option<String>,
    flash_attn: bool,
    diffusion: Option<DiffusionLoadOptions>,
}

fn build_backend_load_spec(
    backend_id: RuntimeBackendId,
    model_path: &str,
    options: BackendLoadSpecOptions,
) -> Result<RuntimeBackendLoadSpec, AppCoreError> {
    let model_path = PathBuf::from(model_path);
    let BackendLoadSpecOptions {
        num_workers,
        context_length,
        chat_template,
        gbnf,
        flash_attn,
        diffusion,
    } = options;

    match backend_id {
        RuntimeBackendId::GgmlLlama => Ok(RuntimeBackendLoadSpec::GgmlLlama(GgmlLlamaLoadConfig {
            model_path,
            num_workers: usize::try_from(num_workers).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to convert num_workers into usize for ggml.llama: {error}"
                ))
            })?,
            context_length: (context_length > 0).then_some(context_length),
            flash_attn,
            chat_template,
            gbnf,
        })),
        RuntimeBackendId::GgmlWhisper => {
            Ok(RuntimeBackendLoadSpec::GgmlWhisper(GgmlWhisperLoadConfig {
                model_path,
                flash_attn,
            }))
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
        RuntimeBackendId::CandleLlama => {
            Ok(RuntimeBackendLoadSpec::CandleLlama(CandleLlamaLoadConfig {
                model_path,
                tokenizer_path: None,
                device: None,
                seed: 0,
            }))
        }
        RuntimeBackendId::CandleWhisper => {
            Ok(RuntimeBackendLoadSpec::CandleWhisper(CandleWhisperLoadConfig {
                model_path,
                tokenizer_path: None,
                device: None,
            }))
        }
        RuntimeBackendId::CandleDiffusion => {
            let diffusion = diffusion.unwrap_or_default();
            Ok(RuntimeBackendLoadSpec::CandleDiffusion(CandleDiffusionLoadConfig {
                model_path,
                vae_path: diffusion.vae_path,
                device: None,
                sd_version: "v2-1".to_owned(),
            }))
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
        if let Some(pack_target) =
            build_catalog_model_pack_load_target(state, &model, &model_path).await?
        {
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
    ensure_runtime_backend_available(state, backend_id)?;

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

async fn build_catalog_model_pack_load_target(
    state: &ModelState,
    model: &UnifiedModel,
    model_path: &str,
) -> Result<Option<model_packs::ModelPackLoadTarget>, AppCoreError> {
    let Some(pack_path) =
        catalog_model_pack_path(state.config().model_config_dir.as_path(), model, model_path)
    else {
        return Ok(None);
    };

    build_selected_model_pack_load_target(state, &model.id, &pack_path).await.map(Some)
}

fn catalog_model_pack_path(
    model_config_dir: &Path,
    model: &UnifiedModel,
    model_path: &str,
) -> Option<PathBuf> {
    if model_packs::is_model_pack_path(model_path) {
        return Some(PathBuf::from(model_path));
    }

    let pack_path = model_packs::model_pack_file_path(model_config_dir, &model.id);
    pack_path.exists().then_some(pack_path)
}

async fn build_selected_model_pack_load_target(
    state: &ModelState,
    model_id: &str,
    pack_path: &Path,
) -> Result<model_packs::ModelPackLoadTarget, AppCoreError> {
    let pack = model_packs::open_model_pack(pack_path)?;
    let resolved = pack.resolve().map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to resolve model pack '{}': {error}",
            pack_path.display()
        ))
    })?;
    let persisted = pack::read_model_download_state_from_db(state, model_id)
        .await?
        .or(model_packs::read_persisted_model_config_from_pack(pack_path)?);
    let state_record = state.store().get_model_config_state(model_id).await?;
    let explicit_selection = if let Some(record) = state_record.as_ref() {
        crate::domain::models::ModelPackSelection {
            preset_id: catalog::normalize_optional_text(record.selected_preset_id.clone()),
            variant_id: catalog::normalize_optional_text(record.selected_variant_id.clone()),
        }
    } else {
        crate::domain::models::ModelPackSelection::default()
    };
    let (effective_selection, selected_preset, _) =
        pack::resolve_effective_model_pack_selection(&resolved, &explicit_selection)?;

    let mut bridge = resolved.compile_runtime_bridge(&selected_preset).map_err(|error| {
        AppCoreError::BadRequest(format!(
            "failed to compile selected pack preset for load target: {error}"
        ))
    })?;
    pack::apply_materialized_source_to_bridge(
        &mut bridge,
        persisted.as_ref(),
        selected_preset.variant.effective_sources.first(),
    );
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
        model_path: load_spec.model_path().to_string_lossy().into_owned(),
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
        ensure_runtime_backend_available(state, backend_id)?;
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
    ensure_runtime_backend_available(state, backend_id)?;
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

#[cfg(test)]
mod tests;
