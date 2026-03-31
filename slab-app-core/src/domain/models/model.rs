use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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
    pub model_id: Option<String>,
    pub backend_id: Option<String>,
    pub model_path: Option<String>,
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
