use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;
use tonic::Status;

mod diffusion;
mod llama;
mod whisper;

#[derive(Default)]
pub struct GrpcServiceImpl;

pub(super) fn runtime_to_status(err: slab_core::RuntimeError) -> Status {
    match err {
        slab_core::RuntimeError::NotInitialized => Status::failed_precondition(err.to_string()),
        other => Status::internal(other.to_string()),
    }
}

pub(super) async fn load_model_for_backend(
    backend: Backend,
    req: pb::ModelLoadRequest,
) -> Result<pb::ModelStatusResponse, Status> {
    if req.model_path.is_empty() {
        return Err(Status::invalid_argument("model_path must not be empty"));
    }
    if req.num_workers == 0 {
        return Err(Status::invalid_argument("num_workers must be at least 1"));
    }

    slab_core::api::backend(backend)
        .load_model()
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path": req.model_path,
            "num_workers": req.num_workers,
        })))
        .run()
        .await
        .map_err(runtime_to_status)?;

    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "loaded".to_string(),
    })
}

pub(super) async fn unload_model_for_backend(
    backend: Backend,
) -> Result<pb::ModelStatusResponse, Status> {
    slab_core::api::backend(backend)
        .unload_model()
        .input(slab_core::Payload::default())
        .run()
        .await
        .map_err(runtime_to_status)?;

    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "unloaded".to_string(),
    })
}

pub(super) async fn reload_library_for_backend(
    backend: Backend,
    req: pb::ReloadLibraryRequest,
) -> Result<pb::ModelStatusResponse, Status> {
    if req.lib_path.is_empty() {
        return Err(Status::invalid_argument("lib_path must not be empty"));
    }
    if req.model_path.is_empty() {
        return Err(Status::invalid_argument("model_path must not be empty"));
    }
    if req.num_workers == 0 {
        return Err(Status::invalid_argument("num_workers must be at least 1"));
    }

    slab_core::api::reload_library(backend, &req.lib_path)
        .await
        .map_err(runtime_to_status)?;

    slab_core::api::backend(backend)
        .load_model()
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path": req.model_path,
            "num_workers": req.num_workers,
        })))
        .run()
        .await
        .map_err(runtime_to_status)?;

    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "loaded".to_string(),
    })
}
