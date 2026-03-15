use tokio::sync::oneshot;

use crate::base::types::Payload;
pub use crate::base::types::{StreamChunk, StreamHandle};

/// Canonical management events supported by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagementEvent {
    Initialize,
    LoadModel,
    UnloadModel,
}

/// Peer-synchronization commands broadcast between workers of the same backend.
///
/// Unlike [`BackendRequest`] (which is competitive – only one worker
/// handles each message), these commands are delivered to every worker
/// simultaneously via a `tokio::sync::broadcast` channel.
///
/// Each variant carries a `sender_id` field identifying the worker that sent
/// the command. Every worker **skips** commands whose `sender_id` matches its
/// own ID because it already performed the operation while handling the
/// original mpsc request — re-processing the command would cause duplicate
/// operations on the sender.
#[derive(Clone, Debug)]
pub enum PeerWorkerCommand {
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

impl PeerWorkerCommand {
    /// Worker id that originally emitted this peer command.
    pub fn sender_id(&self) -> usize {
        match self {
            Self::LoadLibrary { sender_id, .. }
            | Self::ReloadLibrary { sender_id, .. }
            | Self::LoadModel { sender_id, .. }
            | Self::Unload { sender_id, .. } => *sender_id,
        }
    }

    /// Monotonic sequence number assigned by runtime management path.
    pub fn seq_id(&self) -> u64 {
        match self {
            Self::LoadLibrary { seq_id, .. }
            | Self::ReloadLibrary { seq_id, .. }
            | Self::LoadModel { seq_id, .. }
            | Self::Unload { seq_id, .. } => *seq_id,
        }
    }
}

/// Runtime-issued control signals sharing the same backend control bus.
///
/// These are emitted by orchestrator-level global operations, not by peer
/// workers.
#[derive(Clone, Debug)]
pub enum RuntimeControlSignal {
    /// Runtime asks the backend to (re)load state using the provided payload.
    ///
    /// The payload follows backend-specific `model.load`/`lib.load` shape.
    GlobalLoad { op_id: u64, payload: Payload },
    /// Runtime asks the backend to unload all runtime-managed model state.
    GlobalUnload { op_id: u64 },
}

/// Unified control-bus command type for backend worker control channels.
#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Peer(PeerWorkerCommand),
    Runtime(RuntimeControlSignal),
}

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
