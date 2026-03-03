use tokio::sync::{mpsc, oneshot};

use crate::runtime::types::Payload;

/// Broadcast routing scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BroadcastScope {
    /// Broadcast targets workers of a single backend.
    Backend(String),
    /// Broadcast targets all registered backends.
    Global,
}

/// Canonical management events supported by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagementEvent {
    Initialize,
    LoadModel,
    UnloadModel,
}

/// Management envelope sent over control channels.
#[derive(Clone, Debug)]
pub struct ManagementEnvelope {
    pub seq_id: u64,
    pub scope: BroadcastScope,
    pub target_backend: Option<String>,
    pub event: ManagementEvent,
    pub payload: Payload,
    pub origin_worker_id: usize,
    pub issued_at: std::time::SystemTime,
}

/// Management commands broadcast to **all** backend workers so that their
/// internal state stays consistent.
///
/// Unlike [`BackendRequest`] (which is competitive – only one worker
/// handles each message), these commands are delivered to every worker
/// simultaneously via a `tokio::sync::broadcast` channel.
///
/// All stateful operations that mutate the engine (library + model) are
/// broadcast so that every worker reaches the same state regardless of which
/// worker processed the original mpsc request.
///
/// Each variant carries a `sender_id` field identifying the worker that sent
/// the command.  Every worker **skips** commands whose `sender_id` matches its
/// own ID because it already performed the operation while handling the
/// original mpsc request — re-processing the command would cause a double
/// library reload or a redundant unload on the sending worker.
#[derive(Clone, Debug)]
pub enum WorkerCommand {
    /// Load the library from `lib_path` if not already loaded.
    ///
    /// Sent after a `lib.load` request so that peer workers (which did not
    /// handle the original mpsc message) also acquire the library handle.
    LoadLibrary {
        lib_path: String,
        sender_id: usize,
        seq_id: u64,
    },

    /// Drop the current library+model and reload from `lib_path`.
    ///
    /// Sent after a `lib.reload` request so that all workers switch to the
    /// new library together.
    ReloadLibrary {
        lib_path: String,
        sender_id: usize,
        seq_id: u64,
    },

    /// Load the model from `model_path` if not already loaded.
    ///
    /// Sent after a `model.load` request so that peer workers also have a
    /// model context ready for inference.
    LoadModel {
        model_path: String,
        sender_id: usize,
        seq_id: u64,
    },

    /// Drop the current model context on every worker.
    ///
    /// Sent after a `model.unload` request is processed by one worker so
    /// that all other workers also clear their (possibly stale) contexts.
    Unload { sender_id: usize, seq_id: u64 },
}

/// A single chunk emitted by a streaming backend.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A piece of generated output (e.g. a token string).
    Token(String),
    /// Generation completed normally.
    Done,
    /// Generation terminated due to a backend error.
    Error(String),
    /// A generated image (placeholder for now).
    Image(bytes::Bytes), //TODO: A generated image.
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
    /// Arbitrary Payload options forwarded to the backend.
    pub options: Payload,
}

/// Request type used by runtime dispatch to separate management from inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendRequestKind {
    Inference,
    Management(ManagementEvent),
}

/// A request sent by the orchestrator to a backend worker via its ingress queue.
#[derive(Debug)]
pub struct BackendRequest {
    /// Request kind.
    pub kind: BackendRequestKind,
    /// The logical operation to perform.
    pub op: BackendOp,
    /// Input payload for the stage.
    pub input: Payload,
    /// Cancellation signal: watch value becomes `true` when cancelled.
    pub cancel_rx: tokio::sync::watch::Receiver<bool>,
    /// Optional sequence id assigned by the resource manager for management ops.
    pub broadcast_seq: Option<u64>,
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
