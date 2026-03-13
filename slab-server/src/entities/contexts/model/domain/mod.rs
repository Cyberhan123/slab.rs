use chrono::{DateTime, Utc};

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

impl ModelCatalogRecord {
    pub fn mark_downloaded(
        &mut self,
        local_path: String,
        task_id: String,
        downloaded_at: DateTime<Utc>,
    ) {
        self.local_path = Some(local_path);
        self.last_download_task_id = Some(task_id);
        self.last_downloaded_at = Some(downloaded_at);
        self.updated_at = Utc::now();
    }
}
