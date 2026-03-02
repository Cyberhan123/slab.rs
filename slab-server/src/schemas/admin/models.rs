use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateModelRequest {
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateModelRequest {
    pub display_name: Option<String>,
    pub repo_id: Option<String>,
    pub filename: Option<String>,
    pub backend_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModelCatalogResponse {
    pub id: String,
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
    pub local_path: Option<String>,
    pub last_download_task_id: Option<String>,
    pub last_downloaded_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
