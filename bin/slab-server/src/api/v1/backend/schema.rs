use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use slab_app_core::domain::models::BackendStatusView;

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct DownloadLibRequest {
    /// Backend identifier, e.g. `"ggml.llama"`, `"ggml.whisper"`, `"ggml.diffusion"`.
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
    /// Absolute directory where release assets should be installed.
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "target_dir must be an absolute path without '..'"
    ))]
    pub target_dir: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ReloadLibRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "lib_path must be an absolute path without '..'"
    ))]
    pub lib_path: String,
    /// Preferred symmetric reload payload. Mirrors runtime `lib_path + load`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub load: Option<ReloadModelLoadRequest>,
    /// Deprecated flattened compatibility field. Prefer `load.model_path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: Option<String>,
    /// Deprecated flattened compatibility field. Prefer `load.num_workers`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_positive_u32",
        message = "num_workers must be at least 1"
    ))]
    pub num_workers: Option<u32>,
    /// Deprecated flattened compatibility field. Prefer `load.context_length`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_positive_u32",
        message = "context_length must be at least 1"
    ))]
    pub context_length: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ReloadModelLoadRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: String,
    #[serde(default = "default_workers")]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_positive_u32",
        message = "context_length must be at least 1"
    ))]
    pub context_length: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(nested)]
    pub diffusion: Option<ReloadDiffusionLoadOptionsRequest>,
}

#[derive(Debug, Default, Deserialize, ToSchema, Validate)]
pub struct ReloadDiffusionLoadOptionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "diffusion_model_path must be an absolute path without '..'"
    ))]
    pub diffusion_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "vae_path must be an absolute path without '..'"
    ))]
    pub vae_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "taesd_path must be an absolute path without '..'"
    ))]
    pub taesd_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "lora_model_dir must be an absolute path without '..'"
    ))]
    pub lora_model_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "clip_l_path must be an absolute path without '..'"
    ))]
    pub clip_l_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "clip_g_path must be an absolute path without '..'"
    ))]
    pub clip_g_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "t5xxl_path must be an absolute path without '..'"
    ))]
    pub t5xxl_path: Option<String>,
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_device: Option<String>,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
}

fn default_workers() -> u32 {
    1
}

/// Path parameters for model-management routes.
#[derive(Debug, Deserialize, ToSchema, IntoParams, Validate)]
pub struct BackendTypeQuery {
    /// One of `"ggml.llama"`, `"ggml.whisper"`, or `"ggml.diffusion"`.
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
}

/// Response body for load / status endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackendStatusResponse {
    /// Backend identifier, e.g. `"ggml.llama"`.
    pub backend: String,
    /// Human-readable status string.
    pub status: String,
}

/// Response body for list backends endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BackendListResponse {
    pub backends: Vec<BackendStatusResponse>,
}

impl From<BackendStatusView> for BackendStatusResponse {
    fn from(view: BackendStatusView) -> Self {
        Self { backend: view.backend, status: view.status }
    }
}

use std::path::PathBuf;

use slab_app_core::domain::models::{BackendStatusQuery, DownloadBackendLibCommand, ReloadBackendLibCommand};
use slab_types::runtime::{DiffusionLoadOptions, RuntimeModelLoadSpec, RuntimeModelReloadSpec};

use crate::error::ServerError;

impl From<BackendTypeQuery> for BackendStatusQuery {
    fn from(query: BackendTypeQuery) -> Self {
        Self {
            backend_id: query.backend_id.parse().expect("backend_id was validated"),
        }
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
        diffusion: request
            .diffusion
            .as_ref()
            .and_then(diffusion_load_options_from_request),
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
