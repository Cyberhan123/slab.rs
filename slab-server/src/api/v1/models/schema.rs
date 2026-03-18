//! Request / response types for the model-management API (`/v1/models/...`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::domain::models::{
    ModelSpec as DomainModelSpec,
    ModelStatus as DomainModelStatus,
    RuntimePresets as DomainRuntimePresets,
    UnifiedModel as DomainUnifiedModel,
    UnifiedModelStatus as DomainUnifiedModelStatus,
};

// ---------------------------------------------------------------------------
// Nested request schemas
// ---------------------------------------------------------------------------

/// Pricing info for cost tracking.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PricingRequest {
    /// Cost per 1K input tokens in USD.
    pub input: f64,
    /// Cost per 1K output tokens in USD.
    pub output: f64,
}

/// Provider-specific model configuration (request).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct ModelSpecRequest {
    /// Cloud provider settings id from `chat.providers` (e.g. `"openai-main"`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub provider_id: Option<String>,
    /// Remote model identifier for cloud providers (e.g. `"gpt-4o"`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub remote_model_id: Option<String>,
    /// Optional pricing info.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pricing: Option<PricingRequest>,
    /// HuggingFace repo ID for local models.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repo_id: Option<String>,
    /// Filename within the HF repo.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub filename: Option<String>,
    /// Absolute path to the downloaded model file (populated after download).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub local_path: Option<String>,
    /// Maximum context window size in tokens.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub context_window: Option<u32>,
}

/// Default runtime parameters (request).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct RuntimePresetsRequest {
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temperature: Option<f32>,
    /// Top-p nucleus sampling probability.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_p: Option<f32>,
}

// ---------------------------------------------------------------------------
// CRUD request schemas
// ---------------------------------------------------------------------------

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
        message = "provider must not be empty"
    ))]
    pub provider: String,
    /// Initial status. If omitted, defaults to `"ready"` for cloud providers and
    /// `"not_downloaded"` for local providers.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<String>,
    pub spec: Option<ModelSpecRequest>,
    pub runtime_presets: Option<RuntimePresetsRequest>,
}

/// Request body for `POST /v1/models/import`.
///
/// This matches the persisted on-disk model config format. The server stores
/// the uploaded config file under its model config directory and upserts the
/// corresponding row in the unified `models` table.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct ImportModelConfigRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "id must not be empty"
    ))]
    pub id: String,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: String,
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "provider must not be empty"
    ))]
    pub provider: String,
    /// Initial status. If omitted, defaults to `"ready"` for cloud providers and
    /// `"not_downloaded"` for local providers.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<String>,
    #[serde(default)]
    pub spec: ModelSpecRequest,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub runtime_presets: Option<RuntimePresetsRequest>,
}

/// Request body for `PUT /v1/models/{id}`.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct UpdateModelRequest {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub status: Option<String>,
    pub spec: Option<ModelSpecRequest>,
    pub runtime_presets: Option<RuntimePresetsRequest>,
}

// ---------------------------------------------------------------------------
// Load / unload request schemas (unchanged)
// ---------------------------------------------------------------------------

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
    /// Optional worker override.
    #[serde(default)]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: Option<u32>,
}

/// Response body for load / status endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelStatusResponse {
    /// Backend identifier.
    pub backend: String,
    /// Human-readable status string.
    pub status: String,
}

/// Request body for `POST /v1/models/switch`.
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
    #[serde(default)]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: Option<u32>,
}

/// Request body for `POST /v1/models/download`.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct DownloadModelRequest {
    /// Model ID from `/v1/models`.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "model_id must not be empty"
    ))]
    pub model_id: String,
}

/// Query parameters for listing files in a HuggingFace repo.
#[derive(Debug, IntoParams, Deserialize, ToSchema, Validate)]
pub struct ListAvailableQuery {
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "repo_id must not be empty"
    ))]
    pub repo_id: String,
}

// ---------------------------------------------------------------------------
// List query
// ---------------------------------------------------------------------------

/// Query parameters for `GET /v1/models`.
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct ListModelsQuery {
    // Reserved for future filtering (e.g. by provider prefix or status).
}

// ---------------------------------------------------------------------------
// Response schemas
// ---------------------------------------------------------------------------

/// Pricing info in API responses.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PricingResponse {
    pub input: f64,
    pub output: f64,
}

/// Provider-specific model configuration (response).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelSpecResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
}

/// Default runtime parameters (response).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RuntimePresetsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

/// Unified model response returned by `/v1/models`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UnifiedModelResponse {
    pub id: String,
    pub display_name: String,
    /// Provider identifier, e.g. `"cloud.openai"`, `"local.ggml.llama"`.
    pub provider: String,
    /// Status: `"ready"`, `"not_downloaded"`, `"downloading"`, `"error"`.
    pub status: String,
    pub spec: ModelSpecResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_presets: Option<RuntimePresetsResponse>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<DomainModelStatus> for ModelStatusResponse {
    fn from(status: DomainModelStatus) -> Self {
        Self {
            backend: status.backend,
            status: status.status,
        }
    }
}

impl From<DomainModelSpec> for ModelSpecResponse {
    fn from(spec: DomainModelSpec) -> Self {
        Self {
            provider_id: spec.provider_id,
            remote_model_id: spec.remote_model_id,
            pricing: spec.pricing.map(|p| PricingResponse {
                input: p.input,
                output: p.output,
            }),
            repo_id: spec.repo_id,
            filename: spec.filename,
            local_path: spec.local_path,
            context_window: spec.context_window,
        }
    }
}

impl From<DomainRuntimePresets> for RuntimePresetsResponse {
    fn from(presets: DomainRuntimePresets) -> Self {
        Self {
            temperature: presets.temperature,
            top_p: presets.top_p,
        }
    }
}

impl From<DomainUnifiedModel> for UnifiedModelResponse {
    fn from(model: DomainUnifiedModel) -> Self {
        Self {
            id: model.id,
            display_name: model.display_name,
            provider: model.provider,
            status: model.status.as_str().to_owned(),
            spec: model.spec.into(),
            runtime_presets: model.runtime_presets.map(Into::into),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

impl From<DomainUnifiedModelStatus> for String {
    fn from(status: DomainUnifiedModelStatus) -> Self {
        status.as_str().to_owned()
    }
}

