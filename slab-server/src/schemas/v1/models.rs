//! Request / response types for the model-management API (`/api/models/…`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Request body for `POST /api/models/{type}/load`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoadModelRequest {
    /// Backend identifier, e.g. `"ggml.llama"`.
    pub backend_id: String,
    /// Path to the model weights file.
    pub model_path: String,
    /// Number of worker threads to allocate (default `1`).
    #[serde(default = "default_workers")]
    pub num_workers: u32,
}

fn default_workers() -> u32 {
    1
}

/// Response body for load / status endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelStatusResponse {
    /// Backend identifier, e.g. `"ggml.llama"`.
    pub backend: String,
    /// Human-readable status string.
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SwitchModelRequest {
    pub model_path: String,
    pub backend_id: String,
    #[serde(default = "default_workers")]
    pub num_workers: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadModelRequest {
    pub backend_id: String,
    /// HuggingFace repo id, e.g. `"bartowski/Qwen2.5-0.5B-Instruct-GGUF"`.
    pub repo_id: String,
    /// Filename inside the repo to download, e.g. `"Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"`.
    pub filename: String,
    /// Optional directory where the downloaded file will be placed.
    /// If omitted, the hf-hub default cache (`~/.cache/huggingface/hub`) is used.
    pub target_dir: Option<String>,
}

/// Query parameters for listing files in a HuggingFace repo.
#[derive(Debug, IntoParams, Deserialize, ToSchema)]
pub struct ListAvailableQuery {
    /// HuggingFace repo id, e.g. `"bartowski/Qwen2.5-0.5B-Instruct-GGUF"`.
    pub repo_id: String,
}
