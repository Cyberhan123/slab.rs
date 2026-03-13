//! Request / response types for the model-management API (`/v1/models/...`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{
    ModelCatalogItemView as DomainModelCatalogItemView,
    ModelCatalogStatus as DomainModelCatalogStatus, ModelStatus as DomainModelStatus,
};

/// Request body for `POST /v1/models`.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct CreateModelRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: String,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "repo_id must not be empty"
    ))]
    pub repo_id: String,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "filename must not be empty"
    ))]
    pub filename: String,
    #[validate(custom(
        function = "crate::api::validation::validate_backend_ids",
        message = "backend_ids must contain valid backend ids"
    ))]
    pub backend_ids: Vec<String>,
}

/// Request body for `PUT /v1/models/{id}`.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct UpdateModelRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: Option<String>,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "repo_id must not be empty"
    ))]
    pub repo_id: Option<String>,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "filename must not be empty"
    ))]
    pub filename: Option<String>,
    #[validate(custom(
        function = "crate::api::validation::validate_backend_ids",
        message = "backend_ids must contain valid backend ids"
    ))]
    pub backend_ids: Option<Vec<String>>,
}

/// Request body for `POST /v1/models/load`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct LoadModelRequest {
    /// Backend identifier, e.g. `"ggml.llama"`.
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
    /// Path to the model weights file.
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: String,
    /// Optional worker override. If omitted, server uses global config by backend.
    #[serde(default)]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: Option<u32>,
}

/// Response body for load / status endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelStatusResponse {
    /// Backend identifier, e.g. `"ggml.llama"`.
    pub backend: String,
    /// Human-readable status string.
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct SwitchModelRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: String,
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
    /// Optional worker override. If omitted, server uses global config by backend.
    #[serde(default)]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct DownloadModelRequest {
    /// Model catalog entry ID from `/v1/models`.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "model_id must not be empty"
    ))]
    pub model_id: String,
    /// Backend identifier to use for this download.
    #[validate(custom(
        function = "crate::api::validation::validate_backend_id",
        message = "backend_id is invalid"
    ))]
    pub backend_id: String,
}

/// Query parameters for listing files in a HuggingFace repo.
#[derive(Debug, IntoParams, Deserialize, ToSchema, Validate)]
pub struct ListAvailableQuery {
    /// HuggingFace repo id, e.g. `"bartowski/Qwen2.5-0.5B-Instruct-GGUF"`.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "repo_id must not be empty"
    ))]
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
    /// Whether this catalog entry is recognized as a Whisper VAD model candidate.
    pub is_vad_model: bool,
    pub status: ModelListStatus,
    pub local_path: Option<String>,
    pub last_downloaded_at: Option<String>,
    pub pending_task_id: Option<String>,
    pub pending_task_status: Option<String>,
}

impl From<DomainModelStatus> for ModelStatusResponse {
    fn from(status: DomainModelStatus) -> Self {
        Self {
            backend: status.backend,
            status: status.status,
        }
    }
}

impl From<DomainModelCatalogStatus> for ModelListStatus {
    fn from(status: DomainModelCatalogStatus) -> Self {
        match status {
            DomainModelCatalogStatus::Downloaded => Self::Downloaded,
            DomainModelCatalogStatus::Pending => Self::Pending,
            DomainModelCatalogStatus::NotDownloaded => Self::NotDownloaded,
            DomainModelCatalogStatus::All => Self::All,
        }
    }
}

impl From<DomainModelCatalogItemView> for ModelCatalogItemResponse {
    fn from(item: DomainModelCatalogItemView) -> Self {
        Self {
            id: item.id,
            display_name: item.display_name,
            repo_id: item.repo_id,
            filename: item.filename,
            backend_ids: item.backend_ids,
            is_vad_model: item.is_vad_model,
            status: item.status.into(),
            local_path: item.local_path,
            last_downloaded_at: item.last_downloaded_at,
            pending_task_id: item.pending_task_id,
            pending_task_status: item.pending_task_status,
        }
    }
}
