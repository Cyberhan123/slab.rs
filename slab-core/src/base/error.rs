use thiserror::Error;

use crate::base::types::TaskId;

/// Unified error type for the entire `slab-core` crate.
///
/// `CoreError` merges what was previously the scheduler-level `RuntimeError`
/// and the engine-level `EngineError` into a single error hierarchy so that
/// every layer can use one consistent error type without conversion boilerplate.
#[derive(Debug, Clone, Error)]
pub enum CoreError {
    // ── Scheduler errors ────────────────────────────────────────────────────
    /// The ingress queue for the named backend is at capacity.
    #[error("queue full: {queue} (capacity {capacity})")]
    QueueFull { queue: String, capacity: usize },

    /// All admission permits for the backend are held; request denied.
    #[error("backend busy: {backend_id}")]
    Busy { backend_id: String },

    /// The referenced task does not exist.
    #[error("task not found: {task_id}")]
    TaskNotFound { task_id: TaskId },

    /// A CPU stage returned an error.
    #[error("cpu stage '{stage_name}' failed: {message}")]
    CpuStageFailed { stage_name: String, message: String },

    /// A GPU stage returned an error.
    #[error("gpu stage '{stage_name}' failed: {message}")]
    GpuStageFailed { stage_name: String, message: String },

    /// The backend worker channel closed unexpectedly.
    #[error("backend worker shutdown")]
    BackendShutdown,

    /// Orchestrator submission queue is full.
    #[error("orchestrator queue full (capacity {capacity})")]
    OrchestratorQueueFull { capacity: usize },

    /// The runtime was used before it was fully initialized.
    #[error("runtime not initialized")]
    NotInitialized,

    /// A timed wait exceeded its deadline.
    #[error("operation timed out")]
    Timeout,

    /// Failed to load a shared library for a backend.
    #[error("library load failed for backend '{backend}': {message}")]
    LibraryLoadFailed { backend: String, message: String },

    /// The runtime detected split-brain risk after a failed global operation.
    #[error("global state is inconsistent (failed operation {op_id})")]
    GlobalStateInconsistent { op_id: u64 },

    /// Timed out waiting for backend broadcast acknowledgement.
    #[error("broadcast acknowledgement timed out")]
    BroadcastAckTimeout,

    /// Requested operation is not implemented for a backend.
    #[error("unsupported operation '{op}' for backend '{backend}'")]
    UnsupportedOperation { backend: String, op: String },

    /// No failed global operation is available for retry.
    #[error("no failed global operation to retry")]
    NoFailedGlobalOperation,

    /// The supplied model spec could not be validated or normalized.
    #[error("invalid model spec: {message}")]
    InvalidModelSpec { message: String },

    /// The requested model source could not be resolved to local artifacts.
    #[error("model source resolution failed: {message}")]
    SourceResolveFailed { message: String },

    /// None of the registered drivers can satisfy the model and capability.
    #[error("no viable driver for family '{family}' and capability '{capability}'")]
    NoViableDriver { family: String, capability: String },

    /// The model does not expose the requested capability.
    #[error("model family '{family}' does not support capability '{capability}'")]
    UnsupportedCapability { family: String, capability: String },

    /// The requested driver is not registered in the runtime.
    #[error("driver not registered: {driver_id}")]
    DriverNotRegistered { driver_id: String },

    /// Preparing or loading a deployment failed before task execution began.
    #[error("deployment failed for driver '{driver_id}': {message}")]
    DeploymentFailed { driver_id: String, message: String },

    /// A model-scoped operation was attempted before the model had a live deployment.
    #[error("model is not loaded")]
    ModelNotLoaded,

    /// A task result could not be decoded back into the typed public API response.
    #[error("result decode failed for '{task_kind}': {message}")]
    ResultDecodeFailed { task_kind: String, message: String },

    // ── Engine errors ────────────────────────────────────────────────────────
    /// An I/O error raised by an engine backend.
    #[error("engine I/O error: {0}")]
    EngineIo(String),

    /// An error raised by a GGML engine backend.
    #[error("GGML engine error: {0}")]
    GGMLEngine(String),

    /// An error raised by the ONNX Runtime engine backend.
    #[error("ONNX engine error: {0}")]
    OnnxEngine(String),
    /// An error raised by a Candle engine backend.
    #[error("Candle engine error: {0}")]
    CandleEngine(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::EngineIo(e.to_string())
    }
}
