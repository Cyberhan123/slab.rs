//! Request / response types for the model-management API (`/api/models/…`).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for `POST /api/models/{type}/load`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoadModelRequest {
    /// Path to the shared library that implements the backend.
    pub lib_path: String,
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

/// Path parameters for model-management routes.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ModelTypePath {
    /// One of `"llama"`, `"whisper"`, or `"diffusion"`.
    pub model_type: String,
}
