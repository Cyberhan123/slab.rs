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
