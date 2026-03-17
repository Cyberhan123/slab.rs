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

    /// `api::init` was not called before using the API.
    #[error("api runtime not initialized; call api::init first")]
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

    // ── Engine errors ────────────────────────────────────────────────────────
    /// An I/O error raised by an engine backend.
    #[error("engine I/O error: {0}")]
    EngineIo(String),

    /// An error raised by a GGML engine backend.
    #[error("GGML engine error: {0}")]
    GGMLEngine(String),

    /// An error raised by a Candle engine backend.
    #[error("Candle engine error: {0}")]
    CandleEngine(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::EngineIo(e.to_string())
    }
}
