use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{
    BackendStatusQuery, BackendStatusView, DownloadBackendLibCommand,
};

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct DownloadLibRequest {
    /// Backend identifier, e.g. `"ggml.llama"`, `"ggml.whisper"`, `"ggml.diffusion"`.
    #[validate(custom(
        function = "crate::schemas::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
    /// Absolute directory where release assets should be installed.
    #[validate(custom(
        function = "crate::schemas::validation::validate_absolute_path",
        message = "target_dir must be an absolute path without '..'"
    ))]
    pub target_dir: String,
}

/// Path parameters for model-management routes.
#[derive(Debug, Deserialize, ToSchema, IntoParams, Validate)]
pub struct BackendTypeQuery {
    /// One of `"ggml.llama"`, `"ggml.whisper"`, or `"ggml.diffusion"`.
    #[validate(custom(
        function = "crate::schemas::validation::validate_backend_id",
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
