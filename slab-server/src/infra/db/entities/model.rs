use chrono::{DateTime, Utc};

use crate::domain::models::{
    ModelSpec, RuntimePresets, UnifiedModel, UnifiedModelStatus,
};

/// A model entry in the unified `models` table.
/// Both local and cloud models share this structure.
/// `spec` and `runtime_presets` are stored as JSON strings.
#[derive(Debug, Clone)]
pub struct UnifiedModelRecord {
    pub id: String,
    pub display_name: String,
    /// Provider identifier, e.g. `"cloud.openai"`, `"local.ggml.llama"`.
    pub provider: String,
    /// Status string: `"ready"`, `"not_downloaded"`, `"downloading"`, `"error"`.
    pub status: String,
    /// JSON-serialized `ModelSpec`.
    pub spec: String,
    /// JSON-serialized `RuntimePresets`, if any.
    pub runtime_presets: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<UnifiedModelRecord> for UnifiedModel {
    type Error = String;

    fn try_from(record: UnifiedModelRecord) -> Result<Self, Self::Error> {
        let status = record.status.parse::<UnifiedModelStatus>().unwrap_or_else(|error| {
            tracing::warn!(
                id = %record.id,
                raw_status = %record.status,
                error = %error,
                "failed to parse model status; defaulting to Error"
            );
            UnifiedModelStatus::Error
        });

        let spec: ModelSpec = serde_json::from_str(&record.spec).unwrap_or_else(|error| {
            tracing::warn!(
                id = %record.id,
                error = %error,
                "failed to deserialize model spec JSON; using empty spec"
            );
            ModelSpec::default()
        });

        let runtime_presets: Option<RuntimePresets> =
            record.runtime_presets.as_deref().and_then(|value| serde_json::from_str(value).ok());

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
