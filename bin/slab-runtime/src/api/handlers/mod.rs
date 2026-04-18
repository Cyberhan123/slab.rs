use slab_proto::{convert, slab::ipc::v1 as pb};
use slab_runtime_core::CoreError;
use tonic::Status;
use tracing::instrument;

use crate::application::services::{BackendKind, RuntimeApplication, RuntimeApplicationError};
use crate::domain::services::BackendSession;

mod diffusion;
mod llama;
mod whisper;

#[derive(Clone)]
pub struct GrpcServiceImpl {
    application: RuntimeApplication,
}

impl GrpcServiceImpl {
    pub fn new(application: RuntimeApplication) -> Self {
        Self { application }
    }

    pub(super) async fn session_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<BackendSession, Status> {
        self.application.session_for_backend(backend).await.map_err(application_to_status)
    }

    #[instrument(skip_all, fields(backend = backend.canonical_id()))]
    pub(super) async fn load_model_for_backend(
        &self,
        backend: BackendKind,
        request: pb::ModelLoadRequest,
    ) -> Result<pb::ModelStatusResponse, Status> {
        let typed_load_spec =
            convert::decode_model_load_request(&request).map_err(proto_to_status)?;
        let expected_backend = backend.runtime_backend_id();
        let actual_backend = typed_load_spec.backend();
        if actual_backend != expected_backend {
            return Err(Status::invalid_argument(format!(
                "model load payload targets backend '{}' but request was sent to '{}'",
                actual_backend.canonical_id(),
                expected_backend.canonical_id()
            )));
        }
        let status = self
            .application
            .load_model_for_backend(backend, typed_load_spec)
            .await
            .map_err(application_to_status)?;

        Ok(convert::encode_model_status_response(&status))
    }

    #[instrument(skip_all, fields(backend = backend.canonical_id()))]
    pub(super) async fn unload_model_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<pb::ModelStatusResponse, Status> {
        let status = self
            .application
            .unload_model_for_backend(backend)
            .await
            .map_err(application_to_status)?;

        Ok(convert::encode_model_status_response(&status))
    }
}

fn application_to_status(err: RuntimeApplicationError) -> Status {
    match err {
        RuntimeApplicationError::BackendDisabled(backend) => {
            Status::unavailable(format!("{} backend is disabled", backend.canonical_id()))
        }
        RuntimeApplicationError::Runtime(error) => runtime_to_status(error),
    }
}

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

pub(super) fn runtime_to_status(err: CoreError) -> Status {
    let msg = format_error_chain(&err);
    match err {
        CoreError::NotInitialized | CoreError::ModelNotLoaded => Status::failed_precondition(msg),
        CoreError::QueueFull { .. }
        | CoreError::OrchestratorQueueFull { .. }
        | CoreError::Busy { .. } => Status::resource_exhausted(msg),
        CoreError::TaskNotFound { .. } | CoreError::NoFailedGlobalOperation => {
            Status::not_found(msg)
        }
        CoreError::Timeout | CoreError::BroadcastAckTimeout => Status::deadline_exceeded(msg),
        CoreError::Cancelled => Status::cancelled(msg),
        CoreError::BackendShutdown | CoreError::LibraryLoadFailed { .. } => {
            Status::unavailable(msg)
        }
        CoreError::UnsupportedOperation { .. } | CoreError::UnsupportedCapability { .. } => {
            Status::unimplemented(msg)
        }
        CoreError::InvalidModelSpec { .. } | CoreError::SourceResolveFailed { .. } => {
            Status::invalid_argument(msg)
        }
        CoreError::NoViableDriver { .. } | CoreError::DriverNotRegistered { .. } => {
            Status::failed_precondition(msg)
        }
        CoreError::GlobalStateInconsistent { .. }
        | CoreError::CpuStageFailed { .. }
        | CoreError::GpuStageFailed { .. }
        | CoreError::DeploymentFailed { .. }
        | CoreError::ResultDecodeFailed { .. }
        | CoreError::EngineIo(_)
        | CoreError::GGMLEngine(_)
        | CoreError::OnnxEngine(_)
        | CoreError::CandleEngine(_)
        | CoreError::InternalPoisoned { .. } => Status::internal(msg),
    }
}

pub(super) fn proto_to_status(err: convert::ProtoConversionError) -> Status {
    Status::invalid_argument(err.to_string())
}

pub(super) fn extract_request_id(metadata: &tonic::metadata::MetadataMap) -> String {
    metadata
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::runtime_to_status;
    use tonic::Code;

    #[test]
    fn engine_errors_map_to_internal_status() {
        let engine_io =
            runtime_to_status(slab_runtime_core::CoreError::EngineIo("disk offline".into()));
        assert_eq!(engine_io.code(), Code::Internal);
        assert!(engine_io.message().contains("engine I/O error"));

        let ggml =
            runtime_to_status(slab_runtime_core::CoreError::GGMLEngine("session not found".into()));
        assert_eq!(ggml.code(), Code::Internal);
        assert!(ggml.message().contains("GGML engine error"));
    }

    #[test]
    fn cancelled_error_maps_to_cancelled_status() {
        let status = runtime_to_status(slab_runtime_core::CoreError::Cancelled);
        assert_eq!(status.code(), Code::Cancelled);
        assert!(status.message().contains("task cancelled"));
    }
}
