use std::any::Any;
use std::sync::Arc;

use crate::base::error::CoreError;

/// Unique identifier for a submitted pipeline task.
pub type TaskId = u64;

/// Stage-to-stage data transfer type.
///
/// All variants use `Arc` or value types so that moving a `Payload` between
/// stages never copies large buffers.
#[derive(Debug, Clone, Default)]
pub enum Payload {
    #[default]
    None,
    /// Raw bytes (e.g. encoded audio, image data).
    Bytes(Arc<[u8]>),
    /// 32-bit float samples (e.g. PCM audio, embeddings).
    F32(Arc<[f32]>),
    /// UTF-8 text.
    Text(Arc<str>),
    /// Structured JSON metadata.  Not zero-copy but allowed for small objects.
    Json(serde_json::Value),
    /// Escape hatch for arbitrary typed data. Discouraged in core pipelines.
    Any(Arc<dyn Any + Send + Sync>),
}

impl Payload {
    /// Convert to a `serde_json::Value` for use as operation options.
    ///
    /// - `Json` variants are returned as-is.
    /// - `None` returns `serde_json::Value::Null`.
    /// - All other variants return `serde_json::Value::Null`.
    pub fn to_serde_value(&self) -> serde_json::Value {
        match self {
            Payload::Json(v) => v.clone(),
            _ => serde_json::Value::Null,
        }
    }

    pub fn text(s: impl Into<Arc<str>>) -> Self {
        Payload::Text(s.into())
    }

    pub fn bytes(b: impl Into<Arc<[u8]>>) -> Self {
        Payload::Bytes(b.into())
    }

    pub fn f32_slice(f: impl Into<Arc<[f32]>>) -> Self {
        Payload::F32(f.into())
    }

    pub fn json(j: impl Into<serde_json::Value>) -> Self {
        Payload::Json(j.into())
    }

    pub fn to_str_arc(&self) -> Result<Arc<str>, String> {
        match self {
            Payload::Text(t) => Ok(Arc::clone(t)),
            _ => Err(format!("Type error: expected Text variant, got {:?}", self)),
        }
    }

    pub fn to_str(&self) -> Result<&str, String> {
        match self {
            Payload::Text(t) => Ok(t),
            _ => Err(format!("Type error: expected Text variant, got {:?}", self)),
        }
    }

    pub fn to_text_string(&self) -> Result<String, String> {
        match self {
            Payload::Text(t) => Ok(t.to_string()),
            _ => Err(format!("Type error: expected Text variant, got {:?}", self)),
        }
    }

    pub fn to_f32_arc(&self) -> Result<Arc<[f32]>, String> {
        match self {
            Payload::F32(f) => Ok(Arc::clone(f)),
            _ => Err(format!("Type error: expected F32 variant, got {:?}", self)),
        }
    }

    pub fn to_f32_slice(&self) -> Result<&[f32], String> {
        match self {
            Payload::F32(f) => Ok(f),
            _ => Err(format!("Type error: expected F32 variant, got {:?}", self)),
        }
    }

    pub fn to_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        match self {
            Payload::Json(v) => serde_json::from_value(v.clone())
                .map_err(|e| format!("JSON Deserialize error: {e}")),
            _ => Err(format!("Type error: expected Json variant, got {:?}", self)),
        }
    }

    pub fn to_bytes(&self) -> Result<bytes::Bytes, String> {
        match self {
            Payload::Bytes(b) => Ok(bytes::Bytes::copy_from_slice(b)),
            _ => Err(format!(
                "Type error: expected Bytes variant, got {:?}",
                self
            )),
        }
    }
}

impl From<Vec<u8>> for Payload {
    fn from(v: Vec<u8>) -> Self {
        Payload::Bytes(Arc::from(v))
    }
}

impl From<Vec<f32>> for Payload {
    fn from(v: Vec<f32>) -> Self {
        Payload::F32(Arc::from(v))
    }
}

impl From<&str> for Payload {
    fn from(s: &str) -> Self {
        Payload::Text(Arc::from(s))
    }
}

impl From<serde_json::Value> for Payload {
    fn from(v: serde_json::Value) -> Self {
        Payload::Json(v)
    }
}

/// High-level lifecycle state of a task managed by the [`Orchestrator`].
///
/// [`Orchestrator`]: crate::scheduler::orchestrator::Orchestrator
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
    /// Task completed successfully and its result payload has been consumed
    /// by a caller via [`Orchestrator::get_result`].  The task is still
    /// in a terminal (succeeded) state but the inline payload is gone.
    ResultConsumed,
    /// Task completed with a streaming terminal stage; handle is available.
    SucceededStreaming,
    /// Task failed with an error.
    Failed { error: CoreError },
    /// Task was cancelled before completing.
    Cancelled,
}

impl TaskStatus {
    /// Returns `true` if the task has reached a terminal state (success,
    /// streaming-success, result-consumed, failure, or cancellation).
    ///
    /// Callers that poll status until the task is done should use this method
    /// rather than matching individual variants so that new terminal states
    /// (e.g. [`TaskStatus::ResultConsumed`]) are handled automatically.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Succeeded { .. }
                | TaskStatus::ResultConsumed
                | TaskStatus::SucceededStreaming
                | TaskStatus::Failed { .. }
                | TaskStatus::Cancelled
        )
    }

    /// Returns `true` if the task completed with a success outcome.
    ///
    /// This covers [`TaskStatus::Succeeded`], [`TaskStatus::ResultConsumed`]
    /// (succeeded but payload already taken), and [`TaskStatus::SucceededStreaming`].
    pub fn is_succeeded(&self) -> bool {
        matches!(
            self,
            TaskStatus::Succeeded { .. }
                | TaskStatus::ResultConsumed
                | TaskStatus::SucceededStreaming
        )
    }
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

/// A single chunk emitted by a streaming backend.
///
/// Defined here in `base` so that the `ports` interface layer and the
/// scheduler layer can both reference it without a scheduler→ports or
/// ports→scheduler dependency.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated output (e.g. a token string).
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated due to a backend error.
    Error(String),
    /// A generated image (placeholder for now).
    #[allow(dead_code)]
    Image(bytes::Bytes), //TODO: A generated image.
}

/// A handle to a streaming inference response.
///
/// The receiver yields [`StreamChunk`] items as they are produced by the
/// backend worker.  The stream ends with [`StreamChunk::Done`] or
/// [`StreamChunk::Error`].
pub type StreamHandle = tokio::sync::mpsc::Receiver<StreamChunk>;
