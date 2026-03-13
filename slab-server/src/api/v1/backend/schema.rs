use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

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
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: String,
    #[serde(default = "default_workers")]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: u32,
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
