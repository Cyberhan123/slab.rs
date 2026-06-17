//! gRPC handlers are a strict transport boundary.
//!
//! They only perform `pb -> dto -> application -> dto -> pb` forwarding.
//! Compatibility aggregation intentionally does not live here anymore:
//! `<think>` parsing, usage estimation, stop trimming, OpenAI/SSE chunk shaping,
//! whisper plain-text compatibility assembly, and legacy product-contract
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
        CoreError::ModelNotLoaded | CoreError::BackendDisabled { .. } => {
            Status::failed_precondition(msg)
        }
        CoreError::QueueFull { .. }
        | CoreError::OrchestratorQueueFull { .. }
        | CoreError::Busy { .. } => Status::resource_exhausted(msg),
        CoreError::TaskNotFound { .. } => Status::not_found(msg),
        CoreError::Timeout => Status::deadline_exceeded(msg),
        CoreError::Cancelled => Status::cancelled(msg),
        CoreError::BackendShutdown => Status::unavailable(msg),
        CoreError::UnsupportedOperation { .. } => Status::unimplemented(msg),
        CoreError::InvalidRequestPayload { .. } => Status::invalid_argument(msg),
        CoreError::DriverNotRegistered { .. } => Status::failed_precondition(msg),
        CoreError::CpuStageFailed { .. }
        | CoreError::GpuStageFailed { .. }
        | CoreError::ResultDecodeFailed { .. }
        | CoreError::EngineIo(_)
        | CoreError::GGMLEngine { .. }
        | CoreError::OnnxEngine(_)
        | CoreError::CandleEngine { .. }
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

        let ggml = runtime_to_status(CoreError::GGMLEngine {
            component: "ggml.llama".into(),
            message: "session not found".into(),
        });
        assert_eq!(ggml.code(), Code::Internal);
        assert!(ggml.message().contains("GGML engine error"));
        assert!(ggml.message().contains("ggml.llama"));

        let candle = runtime_to_status(CoreError::CandleEngine {
            component: "candle.llama".into(),
            message: "tensor mismatch".into(),
        });
        assert_eq!(candle.code(), Code::Internal);
        assert!(candle.message().contains("Candle engine error"));
        assert!(candle.message().contains("candle.llama"));
    }

    #[test]
    fn cancelled_error_maps_to_cancelled_status() {
        let status = runtime_to_status(CoreError::Cancelled);
        assert_eq!(status.code(), Code::Cancelled);
        assert!(status.message().contains("task cancelled"));
    }

    #[test]
    fn disabled_backend_maps_to_failed_precondition_status() {
        let status = runtime_to_status(CoreError::BackendDisabled { backend: "ggml.llama".into() });
        assert_eq!(status.code(), Code::FailedPrecondition);
        assert!(status.message().contains("disabled"));
    }

    #[test]
    fn maps_all_runtime_error_variants_to_expected_grpc_codes() {
        let cases = vec![
            (
                CoreError::QueueFull { queue: "ingress".into(), capacity: 4 },
                Code::ResourceExhausted,
                "queue full: ingress",
            ),
            (
                CoreError::Busy { backend_id: "ggml.llama".into() },
                Code::ResourceExhausted,
                "backend busy: ggml.llama",
            ),
            (CoreError::TaskNotFound { task_id: 42 }, Code::NotFound, "task not found: 42"),
            (
                CoreError::CpuStageFailed {
                    stage_name: "tokenize".into(),
                    message: "bad vocab".into(),
                },
                Code::Internal,
                "cpu stage 'tokenize' failed",
            ),
            (
                CoreError::GpuStageFailed {
                    stage_name: "decode".into(),
                    message: "device lost".into(),
                },
                Code::Internal,
                "gpu stage 'decode' failed",
            ),
            (CoreError::BackendShutdown, Code::Unavailable, "backend worker shutdown"),
            (
                CoreError::OrchestratorQueueFull { capacity: 16 },
                Code::ResourceExhausted,
                "orchestrator queue full",
            ),
            (CoreError::Timeout, Code::DeadlineExceeded, "operation timed out"),
            (CoreError::Cancelled, Code::Cancelled, "task cancelled"),
            (
                CoreError::UnsupportedOperation {
                    backend: "ggml.llama".into(),
                    op: "embed".into(),
                },
                Code::Unimplemented,
                "unsupported operation 'embed'",
            ),
            (
                CoreError::InvalidRequestPayload { message: "missing prompt".into() },
                Code::InvalidArgument,
                "invalid request payload: missing prompt",
            ),
            (
                CoreError::DriverNotRegistered { driver_id: "onnx".into() },
                Code::FailedPrecondition,
                "driver not registered: onnx",
            ),
            (
                CoreError::BackendDisabled { backend: "onnx".into() },
                Code::FailedPrecondition,
                "backend 'onnx' is disabled",
            ),
            (
                CoreError::InternalPoisoned { lock_name: "resource_manager".into() },
                Code::Internal,
                "internal lock poisoned: resource_manager",
            ),
            (CoreError::ModelNotLoaded, Code::FailedPrecondition, "model is not loaded"),
            (
                CoreError::ResultDecodeFailed {
                    task_kind: "chat".into(),
                    message: "unexpected shape".into(),
                },
                Code::Internal,
                "result decode failed for 'chat'",
            ),
            (
                CoreError::EngineIo("disk offline".into()),
                Code::Internal,
                "engine I/O error: disk offline",
            ),
            (
                CoreError::GGMLEngine {
                    component: "ggml.llama".into(),
                    message: "session missing".into(),
                },
                Code::Internal,
                "GGML engine error in ggml.llama",
            ),
            (
                CoreError::OnnxEngine("provider mismatch".into()),
                Code::Internal,
                "ONNX engine error: provider mismatch",
            ),
            (
                CoreError::CandleEngine {
                    component: "candle.llama".into(),
                    message: "tensor mismatch".into(),
                },
                Code::Internal,
                "Candle engine error in candle.llama",
            ),
        ];

        for (error, expected_code, expected_message) in cases {
            let status = runtime_to_status(error);
            assert_eq!(status.code(), expected_code);
            assert!(
                status.message().contains(expected_message),
                "expected `{}` to contain `{expected_message}`",
                status.message()
            );
        }
    }
}
