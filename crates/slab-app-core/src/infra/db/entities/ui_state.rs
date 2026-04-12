use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct UiStateRecord {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}
