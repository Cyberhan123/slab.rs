//! Request / response types for the model-management API (`/v1/models/...`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Request body for `POST /v1/models/load`.
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
    /// Model catalog entry ID from `/admin/models`.
    pub model_id: String,
    /// Backend identifier to use for this download.
    pub backend_id: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelListStatus {
    Downloaded,
    Pending,
    NotDownloaded,
    All,
}

impl Default for ModelListStatus {
    fn default() -> Self {
        Self::All
    }
}

/// Query parameters for listing catalog models by computed status.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct ListModelsQuery {
    #[serde(default)]
    pub status: ModelListStatus,
}

/// Model catalog entry response with computed download status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelCatalogItemResponse {
    pub id: String,
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
    pub status: ModelListStatus,
    pub local_path: Option<String>,
    pub last_downloaded_at: Option<String>,
    pub pending_task_id: Option<String>,
    pub pending_task_status: Option<String>,
}
