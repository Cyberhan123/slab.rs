use std::any::Any;
use std::sync::Arc;
use thiserror::Error;

/// Unique identifier for a submitted pipeline task.
pub type TaskId = u64;

/// Stage-to-stage data transfer type.
///
/// All variants use `Arc` or value types so that moving a `Payload` between
/// stages never copies large buffers.
#[derive(Debug, Clone)]
pub enum Payload {
    /// Raw bytes (e.g. encoded audio, image data).
    Bytes(Arc<[u8]>),
    /// 32-bit float samples (e.g. PCM audio, embeddings).
    F32(Arc<[f32]>),
    /// UTF-8 text.
    Text(Arc<str>),
    /// Structured JSON metadata.  Not zero-copy but allowed for small objects.
    Json(serde_json::Value),
    /// Escape hatch for arbitrary typed data.  Discouraged in core pipelines.
    Any(Arc<dyn Any + Send + Sync>),
}

/// High-level lifecycle state of a task managed by the [`Orchestrator`].
///
/// [`Orchestrator`]: crate::runtime::orchestrator::Orchestrator
#[derive(Debug, Clone)]
pub enum TaskStatus {
    /// Task has been accepted but not yet started.
    Pending,
    /// Task is actively executing the named stage.
    Running {
        stage_index: usize,
        stage_name: String,
    },
    /// Task completed successfully; result is available.
    Succeeded { result: Payload },
    /// Task completed with a streaming terminal stage; handle is available.
    SucceededStreaming,
    /// Task failed with an error.
    Failed { error: RuntimeError },
    /// Task was cancelled before completing.
    Cancelled,
}

/// Fine-grained execution status of a single pipeline stage.
#[derive(Debug, Clone)]
pub enum StageStatus {
    StagePending,
    StageRunning,
    StageCompleted,
    StageFailed,
    StageCancelled,
}

/// Errors produced by the runtime layer.
#[derive(Debug, Clone, Error)]
pub enum RuntimeError {
    /// The ingress queue for the named queue is at capacity.
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
}
