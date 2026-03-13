use chrono::{DateTime, Utc};

/// A model entry in the `model_catalog` table and related backend mappings.
#[derive(Debug, Clone)]
pub struct ModelCatalogRecord {
    pub id: String,
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
    pub local_path: Option<String>,
    pub last_download_task_id: Option<String>,
    pub last_downloaded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
