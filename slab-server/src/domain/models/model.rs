use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::api::v1::models::schema::{
    CreateModelRequest, ImportModelConfigRequest, ListModelsQuery, LoadModelRequest,
    SwitchModelRequest, UpdateModelRequest,
};
use crate::infra::db::UnifiedModelRecord;

// ---------------------------------------------------------------------------
// Status enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedModelStatus {
    Ready,
    NotDownloaded,
    Downloading,
    Error,
}

impl UnifiedModelStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NotDownloaded => "not_downloaded",
            Self::Downloading => "downloading",
            Self::Error => "error",
        }
    }
}

impl FromStr for UnifiedModelStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ready" => Ok(Self::Ready),
            "not_downloaded" => Ok(Self::NotDownloaded),
            "downloading" => Ok(Self::Downloading),
            "error" => Ok(Self::Error),
            other => Err(format!("unknown model status: {other}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Spec and runtime presets (shared with JSON schema)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelSpec {
    /// Cloud provider settings id from `chat.providers` (e.g. `"openai-main"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// Remote model identifier used by cloud providers (e.g. `"gpt-4o"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_model_id: Option<String>,
    /// Optional pricing info for cost tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Pricing>,
    /// HuggingFace repo ID for local models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// Filename within the HF repo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Absolute path to the downloaded model file (populated after download).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    /// Maximum context window size in tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    /// Optional chat prompt template name for local chat rendering.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pricing {
    pub input: f64,
    pub output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimePresets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

// ---------------------------------------------------------------------------
// Unified domain model view
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct UnifiedModel {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub status: UnifiedModelStatus,
    pub spec: ModelSpec,
    pub runtime_presets: Option<RuntimePresets>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CreateModelCommand {
    pub id: Option<String>,
    pub display_name: String,
    pub provider: String,
    /// If `None`, the status is inferred from the provider prefix.
    pub status: Option<UnifiedModelStatus>,
    pub spec: ModelSpec,
    pub runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredModelConfig {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<UnifiedModelStatus>,
    #[serde(default)]
    pub spec: ModelSpec,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone)]
pub struct UpdateModelCommand {
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub status: Option<UnifiedModelStatus>,
    pub spec: Option<ModelSpec>,
    pub runtime_presets: Option<RuntimePresets>,
}

#[derive(Debug, Clone, Default)]
pub struct ListModelsFilter {
    // No filters currently; reserved for future use (e.g. provider prefix filter).
}

#[derive(Debug, Clone)]
pub struct ModelLoadCommand {
    pub backend_id: String,
    pub model_path: String,
    pub num_workers: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub backend: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct DeletedModelView {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsQuery {
    pub repo_id: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsView {
    pub repo_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadModelCommand {
    pub model_id: String,
}

// ---------------------------------------------------------------------------
// Conversions from API request types
// ---------------------------------------------------------------------------

impl From<CreateModelRequest> for CreateModelCommand {
    fn from(req: CreateModelRequest) -> Self {
        Self {
            id: None,
            display_name: req.display_name,
            provider: req.provider,
            status: req.status.and_then(|s| s.parse().ok()),
            spec: req.spec.map(Into::into).unwrap_or_default(),
            runtime_presets: req.runtime_presets.map(Into::into),
        }
    }
}

impl From<ImportModelConfigRequest> for CreateModelCommand {
    fn from(req: ImportModelConfigRequest) -> Self {
        Self {
            id: Some(req.id),
            display_name: req.display_name,
            provider: req.provider,
            status: req.status.and_then(|s| s.parse().ok()),
            spec: req.spec.into(),
            runtime_presets: req.runtime_presets.map(Into::into),
        }
    }
}

impl From<UpdateModelRequest> for UpdateModelCommand {
    fn from(req: UpdateModelRequest) -> Self {
        Self {
            display_name: req.display_name,
            provider: req.provider,
            status: req.status.and_then(|s| s.parse().ok()),
            spec: req.spec.map(Into::into),
            runtime_presets: req.runtime_presets.map(Into::into),
        }
    }
}

impl From<LoadModelRequest> for ModelLoadCommand {
    fn from(request: LoadModelRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<SwitchModelRequest> for ModelLoadCommand {
    fn from(request: SwitchModelRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<ListModelsQuery> for ListModelsFilter {
    fn from(_query: ListModelsQuery) -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// Conversions from/to DB record
// ---------------------------------------------------------------------------

impl TryFrom<UnifiedModelRecord> for UnifiedModel {
    type Error = String;

    fn try_from(record: UnifiedModelRecord) -> Result<Self, Self::Error> {
        let status = record.status.parse::<UnifiedModelStatus>().unwrap_or_else(|e| {
            tracing::warn!(
                id = %record.id,
                raw_status = %record.status,
                error = %e,
                "failed to parse model status; defaulting to Error"
            );
            UnifiedModelStatus::Error
        });

        let spec: ModelSpec = serde_json::from_str(&record.spec).unwrap_or_else(|e| {
            tracing::warn!(
                id = %record.id,
                error = %e,
                "failed to deserialize model spec JSON; using empty spec"
            );
            ModelSpec::default()
        });

        let runtime_presets: Option<RuntimePresets> =
            record.runtime_presets.as_deref().and_then(|s| serde_json::from_str(s).ok());

        Ok(UnifiedModel {
            id: record.id,
            display_name: record.display_name,
            provider: record.provider,
            status,
            spec,
            runtime_presets,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Spec / RuntimePresets conversion from API schema types
// ---------------------------------------------------------------------------

impl From<crate::api::v1::models::schema::ListAvailableQuery> for AvailableModelsQuery {
    fn from(query: crate::api::v1::models::schema::ListAvailableQuery) -> Self {
        Self { repo_id: query.repo_id }
    }
}

impl From<crate::api::v1::models::schema::DownloadModelRequest> for DownloadModelCommand {
    fn from(req: crate::api::v1::models::schema::DownloadModelRequest) -> Self {
        Self { model_id: req.model_id }
    }
}

impl From<crate::api::v1::models::schema::ModelSpecRequest> for ModelSpec {
    fn from(req: crate::api::v1::models::schema::ModelSpecRequest) -> Self {
        Self {
            provider_id: req.provider_id,
            remote_model_id: req.remote_model_id,
            pricing: req.pricing.map(|p| Pricing { input: p.input, output: p.output }),
            repo_id: req.repo_id,
            filename: req.filename,
            local_path: req.local_path,
            context_window: req.context_window,
            chat_template: req.chat_template,
        }
    }
}

impl From<crate::api::v1::models::schema::RuntimePresetsRequest> for RuntimePresets {
    fn from(req: crate::api::v1::models::schema::RuntimePresetsRequest) -> Self {
        Self { temperature: req.temperature, top_p: req.top_p }
    }
}

impl From<StoredModelConfig> for CreateModelCommand {
    fn from(config: StoredModelConfig) -> Self {
        Self {
            id: Some(config.id),
            display_name: config.display_name,
            provider: config.provider,
            status: config.status,
            spec: config.spec,
            runtime_presets: config.runtime_presets,
        }
    }
}

impl From<UnifiedModel> for StoredModelConfig {
    fn from(model: UnifiedModel) -> Self {
        Self {
            id: model.id,
            display_name: model.display_name,
            provider: model.provider,
            status: Some(model.status),
            spec: model.spec,
            runtime_presets: model.runtime_presets,
        }
    }
}
