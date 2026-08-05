use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ModelConfigStateRecord {
    pub model_id: String,
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub selected_engine_id: Option<String>,
    /// User overrides for pack load parameters (JSON object string, e.g.
    /// `{"num_workers":4}`). `None`/absent means "use pack defaults".
    pub load_overrides: Option<String>,
    /// User overrides for pack inference parameters (JSON object string, e.g.
    /// `{"temperature":0.6}`). `None`/absent means "use pack defaults".
    pub inference_overrides: Option<String>,
    pub updated_at: DateTime<Utc>,
}
