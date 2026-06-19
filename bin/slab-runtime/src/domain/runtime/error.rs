use thiserror::Error;

use super::types::TaskId;

#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    #[error("queue full: {queue} (capacity {capacity})")]
    QueueFull { queue: String, capacity: usize },

    #[error("backend busy: {backend_id}")]
    Busy { backend_id: String },

    #[error("task not found: {task_id}")]
    TaskNotFound { task_id: TaskId },

    #[error("cpu stage '{stage_name}' failed: {message}")]
    CpuStageFailed { stage_name: String, message: String },

    #[error("gpu stage '{stage_name}' failed: {message}")]
    GpuStageFailed { stage_name: String, message: String },

    #[error("backend worker shutdown")]
    BackendShutdown,

    #[error("orchestrator queue full (capacity {capacity})")]
    OrchestratorQueueFull { capacity: usize },

    #[error("operation timed out")]
    Timeout,

    #[error("task cancelled")]
    Cancelled,

    #[error("unsupported operation '{op}' for backend '{backend}'")]
    UnsupportedOperation { backend: String, op: String },

    #[error("invalid request payload: {message}")]
    InvalidRequestPayload { message: String },

    #[error("driver not registered: {driver_id}")]
    DriverNotRegistered { driver_id: String },

    #[error("backend '{backend}' is disabled in this runtime process")]
    BackendDisabled { backend: String },

    #[error("internal lock poisoned: {lock_name}")]
    InternalPoisoned { lock_name: String },

    #[error("model is not loaded")]
    ModelNotLoaded,

    #[error("result decode failed for '{task_kind}': {message}")]
    ResultDecodeFailed { task_kind: String, message: String },

    #[error("engine I/O error: {0}")]
    EngineIo(String),

    #[error("GGML engine error in {component}: {message}")]
    GGMLEngine { component: String, message: String },

    #[error("ONNX engine error: {0}")]
    OnnxEngine(String),

    #[error("Candle engine error in {component}: {message}")]
    CandleEngine { component: String, message: String },
}

impl From<slab_runtime_core::CoreError> for RuntimeError {
    fn from(value: slab_runtime_core::CoreError) -> Self {
        match value {
            slab_runtime_core::CoreError::QueueFull { queue, capacity } => {
                Self::QueueFull { queue, capacity }
            }
            slab_runtime_core::CoreError::Busy { backend_id } => Self::Busy { backend_id },
            slab_runtime_core::CoreError::BackendShutdown => Self::BackendShutdown,
            slab_runtime_core::CoreError::Timeout => Self::Timeout,
            slab_runtime_core::CoreError::UnsupportedOperation { backend, op } => {
                Self::UnsupportedOperation { backend, op }
            }
            slab_runtime_core::CoreError::DriverNotRegistered { driver_id } => {
                Self::DriverNotRegistered { driver_id }
            }
            slab_runtime_core::CoreError::InternalPoisoned { lock_name } => {
                Self::InternalPoisoned { lock_name }
            }
            slab_runtime_core::CoreError::EngineIo(message) => Self::EngineIo(message),
            slab_runtime_core::CoreError::GGMLEngine { component, message } => {
                Self::GGMLEngine { component, message }
            }
            slab_runtime_core::CoreError::OnnxEngine(message) => Self::OnnxEngine(message),
            slab_runtime_core::CoreError::CandleEngine { component, message } => {
                Self::CandleEngine { component, message }
            }
        }
    }
}

impl From<std::io::Error> for RuntimeError {
    fn from(value: std::io::Error) -> Self {
        Self::EngineIo(value.to_string())
    }
}

impl RuntimeError {
    pub fn runtime_code(&self) -> &'static str {
        match self {
            Self::QueueFull { .. } => "runtime_queue_full",
            Self::Busy { .. } => "runtime_backend_busy",
            Self::TaskNotFound { .. } => "runtime_task_not_found",
            Self::CpuStageFailed { .. } => "runtime_cpu_stage_failed",
            Self::GpuStageFailed { .. } => "runtime_gpu_stage_failed",
            Self::BackendShutdown => "runtime_backend_shutdown",
            Self::OrchestratorQueueFull { .. } => "runtime_orchestrator_queue_full",
            Self::Timeout => "runtime_timeout",
            Self::Cancelled => "runtime_cancelled",
            Self::UnsupportedOperation { .. } => "runtime_unsupported_operation",
            Self::InvalidRequestPayload { .. } => "runtime_invalid_request_payload",
            Self::DriverNotRegistered { .. } => "runtime_driver_not_registered",
            Self::BackendDisabled { .. } => "runtime_backend_disabled",
            Self::InternalPoisoned { .. } => "runtime_internal_poisoned",
            Self::ModelNotLoaded => "runtime_model_not_loaded",
            Self::ResultDecodeFailed { .. } => "runtime_result_decode_failed",
            Self::EngineIo(_) => "runtime_engine_io",
            Self::GGMLEngine { .. } => "runtime_ggml_engine",
            Self::OnnxEngine(_) => "runtime_onnx_engine",
            Self::CandleEngine { .. } => "runtime_candle_engine",
        }
    }

    pub fn runtime_detail(&self) -> serde_json::Value {
        match self {
            Self::QueueFull { queue, capacity } => serde_json::json!({
                "queue": queue,
                "capacity": capacity,
                "message": self.to_string(),
            }),
            Self::Busy { backend_id } => serde_json::json!({
                "backend_id": backend_id,
                "message": self.to_string(),
            }),
            Self::TaskNotFound { task_id } => serde_json::json!({
                "task_id": task_id,
                "message": self.to_string(),
            }),
            Self::CpuStageFailed { stage_name, message }
            | Self::GpuStageFailed { stage_name, message } => serde_json::json!({
                "stage_name": stage_name,
                "message": message,
            }),
            Self::BackendShutdown | Self::Timeout | Self::Cancelled | Self::ModelNotLoaded => {
                serde_json::json!({
                    "message": self.to_string(),
                })
            }
            Self::OrchestratorQueueFull { capacity } => serde_json::json!({
                "capacity": capacity,
                "message": self.to_string(),
            }),
            Self::UnsupportedOperation { backend, op } => serde_json::json!({
                "backend": backend,
                "operation": op,
                "message": self.to_string(),
            }),
            Self::InvalidRequestPayload { message } => serde_json::json!({
                "message": message,
            }),
            Self::DriverNotRegistered { driver_id } => serde_json::json!({
                "driver_id": driver_id,
                "message": self.to_string(),
            }),
            Self::BackendDisabled { backend } => serde_json::json!({
                "backend": backend,
                "message": self.to_string(),
            }),
            Self::InternalPoisoned { lock_name } => serde_json::json!({
                "lock_name": lock_name,
                "message": self.to_string(),
            }),
            Self::ResultDecodeFailed { task_kind, message } => serde_json::json!({
                "task_kind": task_kind,
                "message": message,
            }),
            Self::EngineIo(message) | Self::OnnxEngine(message) => serde_json::json!({
                "message": message,
            }),
            Self::GGMLEngine { component, message } | Self::CandleEngine { component, message } => {
                serde_json::json!({
                    "component": component,
                    "message": message,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeError;

    #[test]
    fn core_engine_errors_preserve_component_and_message() {
        let ggml = RuntimeError::from(slab_runtime_core::CoreError::GGMLEngine {
            component: "ggml.llama".to_owned(),
            message: "session not found".to_owned(),
        });
        let candle = RuntimeError::from(slab_runtime_core::CoreError::CandleEngine {
            component: "candle.llama".to_owned(),
            message: "tensor mismatch".to_owned(),
        });

        assert!(matches!(
            ggml,
            RuntimeError::GGMLEngine { component, message }
                if component == "ggml.llama" && message == "session not found"
        ));
        assert!(matches!(
            candle,
            RuntimeError::CandleEngine { component, message }
                if component == "candle.llama" && message == "tensor mismatch"
        ));
    }
}
