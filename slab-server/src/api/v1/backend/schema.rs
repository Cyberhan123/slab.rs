use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::BackendStatusView;

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
