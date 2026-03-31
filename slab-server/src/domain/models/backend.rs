use std::path::PathBuf;

use crate::api::v1::backend::schema::{
    BackendTypeQuery, DownloadLibRequest, ReloadDiffusionLoadOptionsRequest, ReloadLibRequest,
    ReloadModelLoadRequest,
};
use crate::error::ServerError;
use slab_types::RuntimeBackendId;
use slab_types::runtime::{DiffusionLoadOptions, RuntimeModelLoadSpec, RuntimeModelReloadSpec};

#[derive(Debug, Clone)]
pub struct BackendStatusQuery {
    pub backend_id: RuntimeBackendId,
}

#[derive(Debug, Clone)]
pub struct DownloadBackendLibCommand {
    pub backend_id: RuntimeBackendId,
    pub target_dir: String,
}

#[derive(Debug, Clone)]
pub struct ReloadBackendLibCommand {
    pub backend_id: RuntimeBackendId,
    pub spec: RuntimeModelReloadSpec,
    pub uses_legacy_flattened_load: bool,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}

impl From<BackendTypeQuery> for BackendStatusQuery {
    fn from(query: BackendTypeQuery) -> Self {
        Self { backend_id: query.backend_id.parse().expect("backend_id was validated") }
    }
}

impl From<DownloadLibRequest> for DownloadBackendLibCommand {
    fn from(request: DownloadLibRequest) -> Self {
        Self {
            backend_id: request.backend_id.parse().expect("backend_id was validated"),
            target_dir: request.target_dir,
        }
    }
}

impl TryFrom<ReloadLibRequest> for ReloadBackendLibCommand {
    type Error = ServerError;

    fn try_from(request: ReloadLibRequest) -> Result<Self, Self::Error> {
        let backend_id = request.backend_id.parse().expect("backend_id was validated");
        let (load, uses_legacy_flattened_load) = runtime_reload_load_from_request(&request)?;

        Ok(Self {
            backend_id,
            spec: RuntimeModelReloadSpec { lib_path: PathBuf::from(request.lib_path), load },
            uses_legacy_flattened_load,
        })
    }
}

fn runtime_reload_load_from_request(
    request: &ReloadLibRequest,
) -> Result<(RuntimeModelLoadSpec, bool), ServerError> {
    let uses_legacy_fields = request.model_path.is_some()
        || request.num_workers.is_some()
        || request.context_length.is_some();

    if let Some(load) = request.load.as_ref() {
        if uses_legacy_fields {
            return Err(ServerError::BadRequest(
                "reload.load cannot be combined with legacy top-level model_path/num_workers/context_length fields"
                    .into(),
            ));
        }

        return Ok((runtime_model_load_spec_from_request(load), false));
    }

    let Some(model_path) = request.model_path.as_ref() else {
        return Err(ServerError::BadRequest(
            "reload.load is required (legacy top-level model_path is accepted temporarily for compatibility)"
                .into(),
        ));
    };

    Ok((
        RuntimeModelLoadSpec {
            model_path: PathBuf::from(model_path),
            num_workers: request.num_workers.unwrap_or(1).max(1),
            context_length: request.context_length,
            diffusion: None,
        },
        true,
    ))
}

fn runtime_model_load_spec_from_request(request: &ReloadModelLoadRequest) -> RuntimeModelLoadSpec {
    RuntimeModelLoadSpec {
        model_path: PathBuf::from(&request.model_path),
        num_workers: request.num_workers,
        context_length: request.context_length,
        diffusion: request.diffusion.as_ref().and_then(diffusion_load_options_from_request),
    }
}

fn diffusion_load_options_from_request(
    request: &ReloadDiffusionLoadOptionsRequest,
) -> Option<DiffusionLoadOptions> {
    let options = DiffusionLoadOptions {
        diffusion_model_path: request.diffusion_model_path.as_deref().map(PathBuf::from),
        vae_path: request.vae_path.as_deref().map(PathBuf::from),
        taesd_path: request.taesd_path.as_deref().map(PathBuf::from),
        lora_model_dir: request.lora_model_dir.as_deref().map(PathBuf::from),
        clip_l_path: request.clip_l_path.as_deref().map(PathBuf::from),
        clip_g_path: request.clip_g_path.as_deref().map(PathBuf::from),
        t5xxl_path: request.t5xxl_path.as_deref().map(PathBuf::from),
        flash_attn: request.flash_attn,
        vae_device: request.vae_device.clone().unwrap_or_default(),
        clip_device: request.clip_device.clone().unwrap_or_default(),
        offload_params_to_cpu: request.offload_params_to_cpu,
    };

    let has_any_value = options.diffusion_model_path.is_some()
        || options.vae_path.is_some()
        || options.taesd_path.is_some()
        || options.lora_model_dir.is_some()
        || options.clip_l_path.is_some()
        || options.clip_g_path.is_some()
        || options.t5xxl_path.is_some()
        || options.flash_attn
        || !options.vae_device.is_empty()
        || !options.clip_device.is_empty()
        || options.offload_params_to_cpu;

    has_any_value.then_some(options)
}
