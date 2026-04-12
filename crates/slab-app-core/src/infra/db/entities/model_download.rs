use chrono::{DateTime, Utc};

use crate::domain::models::TaskStatus;

/// A row in the `model_downloads` table.
#[derive(Debug, Clone)]
pub struct ModelDownloadRecord {
    pub task_id: String,
    pub model_id: String,
    pub repo_id: String,
    pub filename: String,
    pub status: TaskStatus,
    pub error_msg: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
