use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ModelConfigStateRecord {
    pub model_id: String,
    pub selected_preset_id: Option<String>,
    pub selected_variant_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}
