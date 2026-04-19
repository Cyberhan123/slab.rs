//! gRPC handlers are a strict transport boundary.
//!
//! They only perform `pb -> dto -> application -> dto -> pb` forwarding.
//! Compatibility aggregation intentionally does not live here anymore:
//! `<think>` parsing, usage estimation, stop trimming, OpenAI/SSE chunk shaping,
//! whisper plain-text compatibility assembly, and legacy `slab_types` family
//! request/response construction belong to the server/app-core boundary above
//! runtime.

use tonic::Status;

use crate::application::{
    dtos as dto,
    services::{RuntimeApplication, RuntimeApplicationError},
};
use crate::domain::runtime::CoreError;

mod candle_diffusion;
mod candle_transformers;
mod ggml_diffusion;
mod ggml_llama;
mod ggml_whisper;
mod onnx;

#[derive(Clone)]
pub struct GrpcServiceImpl {
    application: RuntimeApplication,
}

impl GrpcServiceImpl {
    pub fn new(application: RuntimeApplication) -> Self {
        Self { application }
    }
}

fn application_to_status(err: RuntimeApplicationError) -> Status {
    match err {
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
        CoreError::NotInitialized
        | CoreError::ModelNotLoaded
        | CoreError::BackendDisabled { .. } => Status::failed_precondition(msg),
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
        CoreError::CpuStageFailed { .. }
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

pub(super) fn proto_to_status(err: dto::ProtoConversionError) -> Status {
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
    use crate::domain::runtime::CoreError;
    use tonic::Code;

    #[test]
    fn engine_errors_map_to_internal_status() {
        let engine_io = runtime_to_status(CoreError::EngineIo("disk offline".into()));
        assert_eq!(engine_io.code(), Code::Internal);
        assert!(engine_io.message().contains("engine I/O error"));

        let ggml = runtime_to_status(CoreError::GGMLEngine("session not found".into()));
        assert_eq!(ggml.code(), Code::Internal);
        assert!(ggml.message().contains("GGML engine error"));
    }

    #[test]
    fn cancelled_error_maps_to_cancelled_status() {
        let status = runtime_to_status(CoreError::Cancelled);
        assert_eq!(status.code(), Code::Cancelled);
        assert!(status.message().contains("task cancelled"));
    }

    #[test]
    fn disabled_backend_maps_to_failed_precondition_status() {
        let status = runtime_to_status(CoreError::BackendDisabled {
            backend: "ggml.llama".into(),
        });
        assert_eq!(status.code(), Code::FailedPrecondition);
        assert!(status.message().contains("disabled"));
    }
}
