//! Request / response types for the model-management API (`/v1/models/...`).

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::{Validate, ValidationError};

use crate::domain::models::{
    AvailableModelsQuery as DomainAvailableModelsQuery,
    CreateModelCommand as DomainCreateModelCommand,
    DownloadModelCommand as DomainDownloadModelCommand, ListModelsFilter as DomainListModelsFilter,
    ModelLoadCommand as DomainModelLoadCommand, ModelSpec as DomainModelSpec,
    ModelStatus as DomainModelStatus, Pricing as DomainPricing,
    RuntimePresets as DomainRuntimePresets, UnifiedModel as DomainUnifiedModel,
    UpdateModelCommand as DomainUpdateModelCommand,
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
    /// Optional prompt template name used for local chat rendering.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub chat_template: Option<String>,
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
        function = "crate::schemas::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: String,
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
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

/// Request body for `PUT /v1/models/{id}`.
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct UpdateModelRequest {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "display_name must not be empty"
    ))]
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub status: Option<String>,
    pub spec: Option<ModelSpecRequest>,
    pub runtime_presets: Option<RuntimePresetsRequest>,
}

// ---------------------------------------------------------------------------
// Load / unload request schemas
// ---------------------------------------------------------------------------

/// Request body for `POST /v1/models/load`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_load_model_request"))]
pub struct LoadModelRequest {
    /// Catalog model id from `/v1/models`. Preferred for local lifecycle operations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_id: Option<String>,
    /// Legacy backend identifier, e.g. `"ggml.llama"`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend_id: Option<String>,
    /// Legacy path to the model weights file.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_path: Option<String>,
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
#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_switch_model_request"))]
pub struct SwitchModelRequest {
    /// Catalog model id from `/v1/models`. Preferred for local lifecycle operations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_id: Option<String>,
    /// Legacy backend identifier, e.g. `"ggml.llama"`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend_id: Option<String>,
    /// Legacy path to the model weights file.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_path: Option<String>,
    #[serde(default)]
    #[validate(range(min = 1, message = "num_workers must be at least 1"))]
    pub num_workers: Option<u32>,
}

/// Request body for `POST /v1/models/unload`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_unload_model_request"))]
pub struct UnloadModelRequest {
    /// Catalog model id from `/v1/models`. Preferred for local lifecycle operations.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_id: Option<String>,
    /// Legacy backend identifier for direct runtime unloads.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub backend_id: Option<String>,
}

/// Request body for `POST /v1/models/download`.
#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct DownloadModelRequest {
    /// Model ID from `/v1/models`.
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "model_id must not be empty"
    ))]
    pub model_id: String,
}

/// Query parameters for listing files in a HuggingFace repo.
#[derive(Debug, IntoParams, Deserialize, ToSchema, Validate)]
pub struct ListAvailableQuery {
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
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
        Self { backend: status.backend, status: status.status }
    }
}

impl From<DomainModelSpec> for ModelSpecResponse {
    fn from(spec: DomainModelSpec) -> Self {
        Self {
            provider_id: spec.provider_id,
            remote_model_id: spec.remote_model_id,
            pricing: spec.pricing.map(|p| PricingResponse { input: p.input, output: p.output }),
            repo_id: spec.repo_id,
            filename: spec.filename,
            local_path: spec.local_path,
            context_window: spec.context_window,
            chat_template: spec.chat_template,
        }
    }
}

impl From<DomainRuntimePresets> for RuntimePresetsResponse {
    fn from(presets: DomainRuntimePresets) -> Self {
        Self { temperature: presets.temperature, top_p: presets.top_p }
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

impl From<ModelSpecRequest> for DomainModelSpec {
    fn from(req: ModelSpecRequest) -> Self {
        Self {
            provider_id: req.provider_id,
            remote_model_id: req.remote_model_id,
            pricing: req.pricing.map(|p| DomainPricing { input: p.input, output: p.output }),
            repo_id: req.repo_id,
            filename: req.filename,
            local_path: req.local_path,
            context_window: req.context_window,
            chat_template: req.chat_template,
        }
    }
}

impl From<RuntimePresetsRequest> for DomainRuntimePresets {
    fn from(req: RuntimePresetsRequest) -> Self {
        Self { temperature: req.temperature, top_p: req.top_p }
    }
}

impl From<CreateModelRequest> for DomainCreateModelCommand {
    fn from(req: CreateModelRequest) -> Self {
        Self {
            id: None,
            display_name: req.display_name,
            provider: req.provider,
            status: req.status.and_then(|status| status.parse().ok()),
            spec: req.spec.map(Into::into).unwrap_or_default(),
            runtime_presets: req.runtime_presets.map(Into::into),
        }
    }
}

impl From<UpdateModelRequest> for DomainUpdateModelCommand {
    fn from(req: UpdateModelRequest) -> Self {
        Self {
            display_name: req.display_name,
            provider: req.provider,
            status: req.status.and_then(|status| status.parse().ok()),
            spec: req.spec.map(Into::into),
            runtime_presets: req.runtime_presets.map(Into::into),
        }
    }
}

impl From<LoadModelRequest> for DomainModelLoadCommand {
    fn from(request: LoadModelRequest) -> Self {
        Self {
            model_id: request.model_id,
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<SwitchModelRequest> for DomainModelLoadCommand {
    fn from(request: SwitchModelRequest) -> Self {
        Self {
            model_id: request.model_id,
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<UnloadModelRequest> for DomainModelLoadCommand {
    fn from(request: UnloadModelRequest) -> Self {
        Self {
            model_id: request.model_id,
            backend_id: request.backend_id,
            model_path: None,
            num_workers: None,
        }
    }
}

impl From<ListModelsQuery> for DomainListModelsFilter {
    fn from(_query: ListModelsQuery) -> Self {
        Self::default()
    }
}

impl From<ListAvailableQuery> for DomainAvailableModelsQuery {
    fn from(query: ListAvailableQuery) -> Self {
        Self { repo_id: query.repo_id }
    }
}

impl From<DownloadModelRequest> for DomainDownloadModelCommand {
    fn from(req: DownloadModelRequest) -> Self {
        Self { model_id: req.model_id }
    }
}

fn validate_load_model_request(request: &LoadModelRequest) -> Result<(), ValidationError> {
    validate_model_lifecycle_request(
        request.model_id.as_deref(),
        request.backend_id.as_deref(),
        request.model_path.as_deref(),
        true,
    )
}

fn validate_switch_model_request(request: &SwitchModelRequest) -> Result<(), ValidationError> {
    validate_model_lifecycle_request(
        request.model_id.as_deref(),
        request.backend_id.as_deref(),
        request.model_path.as_deref(),
        true,
    )
}

fn validate_unload_model_request(request: &UnloadModelRequest) -> Result<(), ValidationError> {
    validate_model_lifecycle_request(
        request.model_id.as_deref(),
        request.backend_id.as_deref(),
        None,
        false,
    )
}

fn validate_model_lifecycle_request(
    model_id: Option<&str>,
    backend_id: Option<&str>,
    model_path: Option<&str>,
    require_model_path: bool,
) -> Result<(), ValidationError> {
    let model_id = trim_non_empty(model_id);
    let backend_id = trim_non_empty(backend_id);
    let model_path = trim_non_empty(model_path);

    if let Some(model_id) = model_id {
        crate::schemas::validation::validate_non_blank(model_id)?;
        return Ok(());
    }

    let Some(backend_id) = backend_id else {
        return Err(validation_error(
            "missing_model_identity",
            if require_model_path {
                "either model_id or backend_id + model_path is required"
            } else {
                "either model_id or backend_id is required"
            },
        ));
    };
    crate::schemas::validation::validate_backend_id(backend_id)?;

    if require_model_path {
        let Some(model_path) = model_path else {
            return Err(validation_error(
                "missing_model_path",
                "model_path is required when model_id is not provided",
            ));
        };
        crate::schemas::validation::validate_absolute_path(model_path)?;
    }

    Ok(())
}

fn trim_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn validation_error(code: &'static str, message: &'static str) -> ValidationError {
    let mut error = ValidationError::new(code);
    error.message = Some(message.into());
    error
}
