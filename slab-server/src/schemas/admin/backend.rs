use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadLibRequest {
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub tag: Option<String>,
    pub target_path: String,
    pub asset_name: Option<String>,
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
