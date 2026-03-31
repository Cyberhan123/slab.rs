use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::base::error::CoreError;

/// Unique identifier for a submitted pipeline task.
pub type TaskId = u64;

/// Stage-to-stage data transfer type.
///
/// All variants use `Arc` or value types so that moving a `Payload` between
/// stages never copies large buffers.
#[derive(Clone, Default)]
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
    /// Type-erased in-process payload for typed internal handoff.
    Typed(TypedPayload),
}

/// Type-erased payload that preserves the original Rust type for later downcast.
pub struct TypedPayload {
    inner: Arc<dyn Any + Send + Sync>,
    type_name: &'static str,
}

impl TypedPayload {
    pub fn new<T: Send + Sync + 'static>(value: T) -> Self {
        Self { inner: Arc::new(value), type_name: std::any::type_name::<T>() }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    pub fn downcast_arc<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        Arc::clone(&self.inner).downcast::<T>().ok()
    }
}

impl Clone for TypedPayload {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner), type_name: self.type_name }
    }
}

impl fmt::Debug for TypedPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedPayload").field("type_name", &self.type_name).finish()
    }
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("None"),
            Self::Bytes(bytes) => f.debug_tuple("Bytes").field(bytes).finish(),
            Self::F32(values) => f.debug_tuple("F32").field(values).finish(),
            Self::Text(text) => f.debug_tuple("Text").field(text).finish(),
            Self::Json(value) => f.debug_tuple("Json").field(value).finish(),
            Self::Typed(payload) => f.debug_tuple("Typed").field(payload).finish(),
        }
    }
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

    pub fn typed<T: Send + Sync + 'static>(value: T) -> Self {
        Payload::Typed(TypedPayload::new(value))
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

    pub fn to_f32_arc(&self) -> Result<Arc<[f32]>, String> {
        match self {
            Payload::F32(f) => Ok(Arc::clone(f)),
            _ => Err(format!("Type error: expected F32 variant, got {:?}", self)),
        }
    }

    pub fn to_typed_arc<T>(&self) -> Result<Arc<T>, String>
    where
        T: serde::de::DeserializeOwned + Send + Sync + 'static,
    {
        match self {
            Payload::Typed(payload) => payload.downcast_arc::<T>().ok_or_else(|| {
                format!(
                    "Type error: expected Typed payload of {}, got {}",
                    std::any::type_name::<T>(),
                    payload.type_name()
                )
            }),
            Payload::Json(value) => serde_json::from_value(value.clone())
                .map(Arc::new)
                .map_err(|e| format!("JSON Deserialize error: {e}")),
            _ => Err(format!(
                "Type error: expected Typed or Json variant compatible with {}, got {:?}",
                std::any::type_name::<T>(),
                self
            )),
        }
    }

    pub fn to_typed<T>(&self) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
    {
        self.to_typed_arc::<T>().map(|value| value.as_ref().clone())
    }

    pub fn to_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        match self {
            Payload::Json(v) => serde_json::from_value(v.clone())
                .map_err(|e| format!("JSON Deserialize error: {e}")),
            _ => Err(format!("Type error: expected Json variant, got {:?}", self)),
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

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::Payload;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestTypedPayload {
        value: String,
    }

    #[test]
    fn typed_payload_clone_preserves_downcast() {
        let payload = Payload::typed(TestTypedPayload { value: "hello".to_owned() });
        let cloned = payload.clone();

        let typed = cloned
            .to_typed::<TestTypedPayload>()
            .expect("cloned typed payload should downcast successfully");

        assert_eq!(typed.value, "hello");
    }

    #[test]
    fn typed_payload_json_fallback_deserializes() {
        let payload = Payload::json(serde_json::json!({ "value": "json" }));

        let typed = payload
            .to_typed::<TestTypedPayload>()
            .expect("json payload should deserialize through typed helper");

        assert_eq!(typed.value, "json");
    }

    #[test]
    fn typed_payload_reports_type_mismatch() {
        let payload = Payload::typed(123usize);
        let error = payload
            .to_typed::<TestTypedPayload>()
            .expect_err("mismatched typed payload should fail");

        assert!(
            error.contains("TestTypedPayload"),
            "error should mention the requested type: {error}"
        );
        assert!(error.contains("usize"), "error should mention the stored type: {error}");
    }

    #[test]
    fn typed_payload_debug_includes_type_name() {
        let payload = Payload::typed(TestTypedPayload { value: "debug".to_owned() });
        let debug = format!("{payload:?}");

        assert!(debug.contains("TypedPayload"), "debug output should mention typed payload");
        assert!(
            debug.contains("TestTypedPayload"),
            "debug output should include the concrete type name"
        );
    }
}

/// High-level lifecycle state of a task managed by the [`Orchestrator`].
///
/// [`Orchestrator`]: crate::internal::scheduler::orchestrator::Orchestrator
#[derive(Debug, Clone)]
pub enum TaskStatus {
    /// Task has been accepted but not yet started.
    Pending,
    /// Task is actively executing the named stage.
    Running { stage_index: usize, stage_name: String },
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
}

/// Fine-grained execution status of a single pipeline stage.
#[derive(Debug, Clone)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
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
