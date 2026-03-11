use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;
use tonic::Status;

mod diffusion;
mod llama;
mod whisper;

#[derive(Default)]
pub struct GrpcServiceImpl;

/// Format a full error-cause chain as a single string.
///
/// Example: `"top-level: cause: root cause"`
fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut msg = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        msg.push_str(": ");
        msg.push_str(&cause.to_string());
        source = cause.source();
    }
    msg
}

/// Map a [`slab_core::RuntimeError`] to the most appropriate gRPC [`Status`].
///
/// Each variant is mapped to the standard gRPC status code that best reflects
/// the failure category so that callers can take meaningful action on the code
/// without having to parse the description string.
pub(super) fn runtime_to_status(err: slab_core::RuntimeError) -> Status {
    let msg = format_error_chain(&err);
    match err {
        slab_core::RuntimeError::NotInitialized => Status::failed_precondition(msg),
        slab_core::RuntimeError::QueueFull { .. } => Status::resource_exhausted(msg),
        slab_core::RuntimeError::OrchestratorQueueFull { .. } => Status::resource_exhausted(msg),
        slab_core::RuntimeError::Busy { .. } => Status::resource_exhausted(msg),
        slab_core::RuntimeError::TaskNotFound { .. } => Status::not_found(msg),
        slab_core::RuntimeError::Timeout => Status::deadline_exceeded(msg),
        slab_core::RuntimeError::BroadcastAckTimeout => Status::deadline_exceeded(msg),
        slab_core::RuntimeError::BackendShutdown => Status::unavailable(msg),
        slab_core::RuntimeError::LibraryLoadFailed { .. } => Status::unavailable(msg),
        slab_core::RuntimeError::UnsupportedOperation { .. } => Status::unimplemented(msg),
        slab_core::RuntimeError::NoFailedGlobalOperation => Status::not_found(msg),
        slab_core::RuntimeError::GlobalStateInconsistent { .. } => Status::internal(msg),
        slab_core::RuntimeError::CpuStageFailed { .. } => Status::internal(msg),
        slab_core::RuntimeError::GpuStageFailed { .. } => Status::internal(msg),
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
            "context_length": req.context_length,
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
            "context_length": req.context_length,
        })))
        .run()
        .await
        .map_err(runtime_to_status)?;

    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "loaded".to_string(),
    })
}
