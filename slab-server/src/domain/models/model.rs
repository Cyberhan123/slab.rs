#[derive(Debug, Clone)]
pub struct ModelLoadCommand {
    pub backend_id: String,
    pub model_path: String,
    pub num_workers: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub backend: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CreateModelCommand {
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateModelCommand {
    pub display_name: Option<String>,
    pub repo_id: Option<String>,
    pub filename: Option<String>,
    pub backend_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCatalogStatus {
    Downloaded,
    Pending,
    NotDownloaded,
    All,
}

#[derive(Debug, Clone)]
pub struct ListModelsFilter {
    pub status: ModelCatalogStatus,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsQuery {
    pub repo_id: String,
}

#[derive(Debug, Clone)]
pub struct AvailableModelsView {
    pub repo_id: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadModelCommand {
    pub model_id: String,
    pub backend_id: String,
}

#[derive(Debug, Clone)]
pub struct ModelCatalogItemView {
    pub id: String,
    pub display_name: String,
    pub repo_id: String,
    pub filename: String,
    pub backend_ids: Vec<String>,
    pub is_vad_model: bool,
    pub status: ModelCatalogStatus,
    pub local_path: Option<String>,
    pub last_downloaded_at: Option<String>,
    pub pending_task_id: Option<String>,
    pub pending_task_status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeletedModelView {
    pub id: String,
    pub status: String,
}
