use tokio::sync::{mpsc, oneshot};

use crate::runtime::types::Payload;

/// A single chunk emitted by a streaming backend.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated output (e.g. a token string).
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated due to a backend error.
    Error(String),
}

/// A handle to a streaming inference response.
///
/// The receiver yields [`StreamChunk`] items as they are produced by the
/// backend worker.  The stream ends with [`StreamChunk::Done`] or
/// [`StreamChunk::Error`].
pub type StreamHandle = mpsc::Receiver<StreamChunk>;

/// Operation identifier passed to a backend in a [`BackendRequest`].
#[derive(Debug, Clone)]
pub struct BackendOp {
    /// Logical operation name (e.g. `"transcribe"`, `"generate"`).
    pub name: String,
    /// Arbitrary JSON options forwarded to the backend.
    pub options: serde_json::Value,
}

/// A request sent by the orchestrator to a backend worker via its ingress queue.
#[derive(Debug)]
pub struct BackendRequest {
    /// The logical operation to perform.
    pub op: BackendOp,
    /// Input payload for the stage.
    pub input: Payload,
    /// Cancellation signal: watch value becomes `true` when cancelled.
    pub cancel_rx: tokio::sync::watch::Receiver<bool>,
    /// Channel on which the backend sends its single reply.
    pub reply_tx: oneshot::Sender<BackendReply>,
}

/// Reply sent back from a backend worker to the orchestrator.
#[derive(Debug)]
pub enum BackendReply {
    /// A single complete output payload (non-streaming).
    Value(Payload),
    /// A streaming output handle (terminal stage only).
    Stream(StreamHandle),
    /// The backend encountered an error.
    Error(String),
}
