use crate::api::v1::models::schema::{
    CreateModelRequest, DownloadModelRequest, ListAvailableQuery, ListModelsQuery,
    LoadModelRequest, ModelListStatus, SwitchModelRequest, UpdateModelRequest,
};
use crate::infra::db::{ModelCatalogRecord, TaskRecord};

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

impl From<CreateModelRequest> for CreateModelCommand {
    fn from(request: CreateModelRequest) -> Self {
        Self {
            display_name: request.display_name,
            repo_id: request.repo_id,
            filename: request.filename,
            backend_ids: request.backend_ids,
        }
    }
}

impl From<UpdateModelRequest> for UpdateModelCommand {
    fn from(request: UpdateModelRequest) -> Self {
        Self {
            display_name: request.display_name,
            repo_id: request.repo_id,
            filename: request.filename,
            backend_ids: request.backend_ids,
        }
    }
}

impl From<LoadModelRequest> for ModelLoadCommand {
    fn from(request: LoadModelRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<SwitchModelRequest> for ModelLoadCommand {
    fn from(request: SwitchModelRequest) -> Self {
        Self {
            backend_id: request.backend_id,
            model_path: request.model_path,
            num_workers: request.num_workers,
        }
    }
}

impl From<DownloadModelRequest> for DownloadModelCommand {
    fn from(request: DownloadModelRequest) -> Self {
        Self {
            model_id: request.model_id,
            backend_id: request.backend_id,
        }
    }
}

impl From<ListAvailableQuery> for AvailableModelsQuery {
    fn from(query: ListAvailableQuery) -> Self {
        Self {
            repo_id: query.repo_id,
        }
    }
}

impl From<ModelListStatus> for ModelCatalogStatus {
    fn from(status: ModelListStatus) -> Self {
        match status {
            ModelListStatus::Downloaded => Self::Downloaded,
            ModelListStatus::Pending => Self::Pending,
            ModelListStatus::NotDownloaded => Self::NotDownloaded,
            ModelListStatus::All => Self::All,
        }
    }
}

impl From<ListModelsQuery> for ListModelsFilter {
    fn from(query: ListModelsQuery) -> Self {
        Self {
            status: query.status.into(),
        }
    }
}

impl From<(ModelCatalogRecord, Option<&TaskRecord>)> for ModelCatalogItemView {
    fn from((model, pending_task): (ModelCatalogRecord, Option<&TaskRecord>)) -> Self {
        let status = if model.local_path.is_some() {
            ModelCatalogStatus::Downloaded
        } else if pending_task.is_some() {
            ModelCatalogStatus::Pending
        } else {
            ModelCatalogStatus::NotDownloaded
        };

        let is_vad_model = detect_whisper_vad_model(
            &model.backend_ids,
            &model.display_name,
            &model.repo_id,
            &model.filename,
        );

        Self {
            id: model.id,
            display_name: model.display_name,
            repo_id: model.repo_id,
            filename: model.filename,
            backend_ids: model.backend_ids,
            is_vad_model,
            status,
            local_path: model.local_path,
            last_downloaded_at: model.last_downloaded_at.map(|value| value.to_rfc3339()),
            pending_task_id: pending_task.map(|task| task.id.clone()),
            pending_task_status: pending_task.map(|task| task.status.clone()),
        }
    }
}

fn detect_whisper_vad_model(
    backend_ids: &[String],
    display_name: &str,
    repo_id: &str,
    filename: &str,
) -> bool {
    if !backend_ids.iter().any(|backend| backend == "ggml.whisper") {
        return false;
    }

    let haystack = format!(
        "{} {} {}",
        display_name.to_ascii_lowercase(),
        repo_id.to_ascii_lowercase(),
        filename.to_ascii_lowercase()
    );

    [
        " vad", "vad ", "-vad", "_vad", "vad-", "vad_", "silero", "fsmn-vad",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
        || haystack.ends_with("vad")
}
