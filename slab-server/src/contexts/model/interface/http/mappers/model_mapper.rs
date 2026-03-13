use crate::contexts::model::domain::{ModelLoadCommand, ModelStatus};
use crate::schemas::v1::models::{LoadModelRequest, ModelStatusResponse};

pub fn to_model_load_command(request: LoadModelRequest) -> ModelLoadCommand {
    ModelLoadCommand {
        backend_id: request.backend_id,
        model_path: request.model_path,
        num_workers: request.num_workers,
    }
}

pub fn to_model_status_response(status: ModelStatus) -> ModelStatusResponse {
    ModelStatusResponse {
        backend: status.backend,
        status: status.status,
    }
}
