use slab_core::api::Backend;
use slab_proto::slab::ipc::v1 as pb;
use tonic::Status;
use tracing::{error, info, instrument, warn};

mod diffusion;
mod llama;
mod whisper;

#[derive(Default)]
pub struct GrpcServiceImpl;

// ---------------------------------------------------------------------------
// Error-chain helpers
// ---------------------------------------------------------------------------

/// Format a full `std::error::Error` cause chain as a single colon-separated
/// string: `"top-level: cause: root cause"`.
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
        slab_core::RuntimeError::EngineIo(_) => Status::internal(msg),
        slab_core::RuntimeError::GGMLEngine(_) => Status::internal(msg),
    }
}

// ---------------------------------------------------------------------------
// Metadata helpers
// ---------------------------------------------------------------------------

/// Extract the `x-request-id` value from incoming gRPC request metadata.
///
/// Returns `"unknown"` when the header is absent or contains non-ASCII bytes.
pub(super) fn extract_request_id(metadata: &tonic::metadata::MetadataMap) -> String {
    metadata
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_owned()
}

// ---------------------------------------------------------------------------
// Shared backend helpers
// ---------------------------------------------------------------------------

#[instrument(skip_all, fields(backend = %backend, model_path = %req.model_path))]
pub(super) async fn load_model_for_backend(
    backend: Backend,
    req: pb::ModelLoadRequest,
) -> Result<pb::ModelStatusResponse, Status> {
    if req.model_path.is_empty() {
        warn!("load_model rejected: model_path is empty");
        return Err(Status::invalid_argument("model_path must not be empty"));
    }
    if req.num_workers == 0 {
        warn!("load_model rejected: num_workers is zero");
        return Err(Status::invalid_argument("num_workers must be at least 1"));
    }

    info!(num_workers = req.num_workers, "loading model");

    slab_core::api::backend(backend)
        .load_model()
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path": req.model_path,
            "num_workers": req.num_workers,
            "context_length": req.context_length,
            // Diffusion-specific context params (ignored by non-diffusion backends).
            "diffusion_model_path": req.diffusion_model_path,
            "vae_path": req.vae_path,
            "taesd_path": req.taesd_path,
            "lora_model_dir": req.lora_model_dir,
            "clip_l_path": req.clip_l_path,
            "clip_g_path": req.clip_g_path,
            "t5xxl_path": req.t5xxl_path,
            "flash_attn": req.flash_attn,
            "keep_vae_on_cpu": req.keep_vae_on_cpu,
            "keep_clip_on_cpu": req.keep_clip_on_cpu,
            "offload_params_to_cpu": req.offload_params_to_cpu,
        })))
        .run()
        .await
        .map_err(|e| {
            error!(error = %e, "load_model backend call failed");
            runtime_to_status(e)
        })?;

    info!("model loaded successfully");
    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "loaded".to_string(),
    })
}

#[instrument(skip_all, fields(backend = %backend))]
pub(super) async fn unload_model_for_backend(
    backend: Backend,
) -> Result<pb::ModelStatusResponse, Status> {
    info!("unloading model");

    slab_core::api::backend(backend)
        .unload_model()
        .input(slab_core::Payload::default())
        .run()
        .await
        .map_err(|e| {
            error!(error = %e, "unload_model backend call failed");
            runtime_to_status(e)
        })?;

    info!("model unloaded successfully");
    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "unloaded".to_string(),
    })
}

#[instrument(skip_all, fields(backend = %backend, lib_path = %req.lib_path, model_path = %req.model_path))]
pub(super) async fn reload_library_for_backend(
    backend: Backend,
    req: pb::ReloadLibraryRequest,
) -> Result<pb::ModelStatusResponse, Status> {
    if req.lib_path.is_empty() {
        warn!("reload_library rejected: lib_path is empty");
        return Err(Status::invalid_argument("lib_path must not be empty"));
    }
    if req.model_path.is_empty() {
        warn!("reload_library rejected: model_path is empty");
        return Err(Status::invalid_argument("model_path must not be empty"));
    }
    if req.num_workers == 0 {
        warn!("reload_library rejected: num_workers is zero");
        return Err(Status::invalid_argument("num_workers must be at least 1"));
    }

    info!(num_workers = req.num_workers, "reloading library");

    slab_core::api::reload_library(backend, &req.lib_path)
        .await
        .map_err(|e| {
            error!(error = %e, "reload_library call failed");
            runtime_to_status(e)
        })?;

    info!("library reloaded; loading model");

    slab_core::api::backend(backend)
        .load_model()
        .input(slab_core::Payload::Json(serde_json::json!({
            "model_path": req.model_path,
            "num_workers": req.num_workers,
            "context_length": req.context_length,
        })))
        .run()
        .await
        .map_err(|e| {
            error!(error = %e, "load_model after reload failed");
            runtime_to_status(e)
        })?;

    info!("library reload and model load completed successfully");
    Ok(pb::ModelStatusResponse {
        backend: backend.to_string(),
        status: "loaded".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::runtime_to_status;
    use tonic::Code;

    #[test]
    fn engine_errors_map_to_internal_status() {
        let engine_io = runtime_to_status(slab_core::RuntimeError::EngineIo("disk offline".into()));
        assert_eq!(engine_io.code(), Code::Internal);
        assert!(engine_io.message().contains("engine I/O error"));

        let ggml = runtime_to_status(slab_core::RuntimeError::GGMLEngine(
            "session not found".into(),
        ));
        assert_eq!(ggml.code(), Code::Internal);
        assert!(ggml.message().contains("GGML engine error"));
    }
}
