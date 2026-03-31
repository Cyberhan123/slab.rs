use std::str::FromStr;

use tokio::sync::oneshot;

use crate::base::types::Payload;
pub use crate::base::types::{StreamChunk, StreamHandle};

/// Typed request route understood by backend workers.
///
/// The scheduler still constructs requests from legacy op strings today, but
/// workers no longer depend on `crate::api::Event` or string matching after
/// the ingress boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequestRoute {
    LoadModel,
    UnloadModel,
    Inference,
    InferenceStream,
    InferenceImage,
}

impl FromStr for RequestRoute {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "model.load" => Ok(Self::LoadModel),
            "model.unload" => Ok(Self::UnloadModel),
            "inference" => Ok(Self::Inference),
            "inference.stream" => Ok(Self::InferenceStream),
            "inference.image" => Ok(Self::InferenceImage),
            other => Err(format!("unknown backend op: {other}")),
        }
    }
}

/// Canonical management events supported by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagementEvent {
    LoadModel,
    UnloadModel,
}

/// Typed inference request metadata derived from a [`BackendRequest`].
#[derive(Debug, Clone)]
pub struct Invocation {
    #[cfg(test)]
    pub route: RequestRoute,
    pub options: Payload,
}

/// Complete deployment state that can be broadcast to peer workers or exposed
/// as a typed runtime-control view.
#[derive(Clone, Debug, Default)]
pub struct DeploymentSnapshot {
    pub generation: u64,
    pub model: Option<Payload>,
}

impl DeploymentSnapshot {
    pub fn with_model(generation: u64, payload: Payload) -> Self {
        Self { generation, model: Some(payload) }
    }

    pub fn model_config<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        self.model
            .as_ref()
            .ok_or_else(|| "deployment snapshot missing model config".to_owned())?
            .to_json()
    }

    pub fn typed_model_config<T>(&self) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned + Clone + Send + Sync + 'static,
    {
        self.model
            .as_ref()
            .ok_or_else(|| "deployment snapshot missing model config".to_owned())?
            .to_typed()
    }
}

/// Typed synchronization payload on the backend control bus.
#[derive(Clone, Debug)]
pub enum SyncMessage {
    Deployment(DeploymentSnapshot),
    Generation { generation: u64 },
}

impl SyncMessage {
    pub fn generation(&self) -> u64 {
        match self {
            Self::Deployment(snapshot) => snapshot.generation,
            Self::Generation { generation } => *generation,
        }
    }

    pub fn deployment(&self) -> Option<&DeploymentSnapshot> {
        match self {
            Self::Deployment(snapshot) => Some(snapshot),
            Self::Generation { .. } => None,
        }
    }
}

/// Peer-synchronization commands broadcast between workers of the same backend.
#[derive(Clone, Debug)]
pub enum PeerWorkerCommand {
    LoadModel { sync: SyncMessage, sender_id: usize },
    Unload { sync: SyncMessage, sender_id: usize },
}

impl PeerWorkerCommand {
    /// Worker id that originally emitted this peer command.
    pub fn sender_id(&self) -> usize {
        match self {
            Self::LoadModel { sender_id, .. } | Self::Unload { sender_id, .. } => *sender_id,
        }
    }

    /// Monotonic sequence number assigned by runtime management path.
    pub fn seq_id(&self) -> u64 {
        self.sync().generation()
    }

    pub fn sync(&self) -> &SyncMessage {
        match self {
            Self::LoadModel { sync, .. } | Self::Unload { sync, .. } => sync,
        }
    }

    pub fn deployment(&self) -> Option<&DeploymentSnapshot> {
        self.sync().deployment()
    }
}

/// Runtime-issued control signals sharing the same backend control bus.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Debug)]
pub enum RuntimeControlSignal {
    /// Runtime asks the backend to (re)load state using the provided payload.
    ///
    /// The payload follows the backend-specific `model.load` shape.
    GlobalLoad { op_id: u64, payload: Payload },
    /// Runtime asks the backend to unload all runtime-managed model state.
    GlobalUnload { op_id: u64 },
}

/// Unified control-bus command type for backend worker control channels.
#[derive(Clone, Debug)]
pub enum WorkerCommand {
    Peer(PeerWorkerCommand),
    #[cfg_attr(not(test), allow(dead_code))]
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

#[cfg(test)]
/// Higher-level typed view over backend ingress requests.
#[derive(Debug, Clone)]
pub enum DriverRequestKind {
    Inference(Invocation),
    Management { event: ManagementEvent },
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

impl BackendRequest {
    pub fn route(&self) -> Result<RequestRoute, String> {
        RequestRoute::from_str(&self.op.name)
    }

    #[cfg(test)]
    pub fn driver_kind(&self) -> Result<DriverRequestKind, String> {
        let route = self.route()?;
        Ok(match self.kind {
            BackendRequestKind::Inference => DriverRequestKind::Inference(Invocation {
                #[cfg(test)]
                route,
                options: self.op.options.clone(),
            }),
            BackendRequestKind::Management(event) => DriverRequestKind::Management { event },
        })
    }

    pub fn invocation(&self) -> Result<Invocation, String> {
        #[cfg(test)]
        let route = self.route()?;
        #[cfg(not(test))]
        let _ = self.route()?;
        Ok(Invocation {
            #[cfg(test)]
            route,
            options: self.op.options.clone(),
        })
    }
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
