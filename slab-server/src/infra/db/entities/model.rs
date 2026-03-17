use chrono::{DateTime, Utc};

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
