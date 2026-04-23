use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct PluginStateRecord {
    pub plugin_id: String,
    pub source_kind: String,
    pub source_ref: Option<String>,
    pub install_root: Option<String>,
    pub installed_version: Option<String>,
    pub manifest_hash: Option<String>,
    pub enabled: bool,
    pub runtime_status: String,
    pub last_error: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_started_at: Option<DateTime<Utc>>,
    pub last_stopped_at: Option<DateTime<Utc>>,
}
