use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadLibRequest {
    /// Backend identifier, e.g. `"ggml.llama"`, `"ggml.whisper"`, `"ggml.diffusion"`.
    pub backend_id: String,
    /// Absolute directory where release assets should be installed.
    pub target_dir: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReloadLibRequest {
    pub backend_id: String,
    pub lib_path: String,
    pub model_path: String,
    #[serde(default = "default_workers")]
    pub num_workers: u32,
}

fn default_workers() -> u32 {
    1
}

/// Path parameters for model-management routes.
#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct BackendTypeQuery {
    /// One of `"ggml.llama"`, `"ggml.whisper"`, or `"ggml.diffusion"`.
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