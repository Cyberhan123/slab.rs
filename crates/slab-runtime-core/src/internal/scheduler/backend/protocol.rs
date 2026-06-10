use std::str::FromStr;

use tokio::sync::{broadcast, oneshot, watch};

use crate::base::types::Payload;
pub use crate::base::types::StreamHandle;

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

impl RequestRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadModel => "model.load",
            Self::UnloadModel => "model.unload",
            Self::Inference => "inference",
            Self::InferenceStream => "inference.stream",
            Self::InferenceImage => "inference.image",
        }
    }
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

/// Discriminant for constructing peer-synchronization commands generically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PeerWorkerCommandKind {
    LoadModel,
    Unload,
}

impl PeerWorkerCommandKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadModel => "LoadModel",
            Self::Unload => "Unload",
        }
    }

    pub fn into_command(self, sync: SyncMessage, sender_id: usize) -> PeerWorkerCommand {
        match self {
            Self::LoadModel => PeerWorkerCommand::LoadModel { sync, sender_id },
            Self::Unload => PeerWorkerCommand::Unload { sync, sender_id },
        }
    }
}

impl PeerWorkerCommand {
    pub fn from_kind(kind: PeerWorkerCommandKind, sync: SyncMessage, sender_id: usize) -> Self {
        kind.into_command(sync, sender_id)
    }

    pub fn kind(&self) -> PeerWorkerCommandKind {
        match self {
            Self::LoadModel { .. } => PeerWorkerCommandKind::LoadModel,
            Self::Unload { .. } => PeerWorkerCommandKind::Unload,
        }
    }

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

/// Worker-facing peer synchronization emitter.
///
/// Backend workers use this instead of constructing [`WorkerCommand`] values or
/// touching the underlying broadcast channel directly.
#[derive(Clone)]
pub struct PeerControlBus {
    tx: broadcast::Sender<WorkerCommand>,
    sender_id: usize,
}

impl PeerControlBus {
    pub fn new(tx: broadcast::Sender<WorkerCommand>, sender_id: usize) -> Self {
        Self { tx, sender_id }
    }

    pub fn sender_id(&self) -> usize {
        self.sender_id
    }

    pub fn broadcast_peer_deployment(
        &self,
        kind: PeerWorkerCommandKind,
        generation: u64,
        model: Payload,
    ) {
        let deployment = DeploymentSnapshot::with_model(generation, model);
        self.broadcast_peer_sync(kind, SyncMessage::Deployment(deployment));
    }

    pub fn broadcast_peer_typed_deployment<T>(
        &self,
        kind: PeerWorkerCommandKind,
        generation: u64,
        model: T,
    ) where
        T: Send + Sync + 'static,
    {
        self.broadcast_peer_deployment(kind, generation, Payload::typed(model));
    }

    pub fn broadcast_peer_generation(&self, kind: PeerWorkerCommandKind, generation: u64) {
        self.broadcast_peer_sync(kind, SyncMessage::Generation { generation });
    }

    pub fn broadcast_peer_sync(&self, kind: PeerWorkerCommandKind, sync: SyncMessage) {
        self.send_peer(PeerWorkerCommand::from_kind(kind, sync, self.sender_id), kind.as_str());
    }

    pub fn broadcast_model_loaded(&self, generation: u64, model: Payload) {
        self.broadcast_peer_deployment(PeerWorkerCommandKind::LoadModel, generation, model);
    }

    pub fn broadcast_typed_model_loaded<T>(&self, generation: u64, model: T)
    where
        T: Send + Sync + 'static,
    {
        self.broadcast_model_loaded(generation, Payload::typed(model));
    }

    pub fn broadcast_model_unloaded(&self, generation: u64) {
        self.broadcast_peer_generation(PeerWorkerCommandKind::Unload, generation);
    }

    fn send_peer(&self, command: PeerWorkerCommand, action: &'static str) {
        let generation = command.seq_id();
        if self.tx.receiver_count() == 0 {
            tracing::debug!(
                sender_id = self.sender_id,
                generation,
                action,
                "peer control broadcast skipped: no receivers"
            );
            return;
        }

        match self.tx.send(WorkerCommand::Peer(command)) {
            Ok(receiver_count) => {
                tracing::trace!(
                    sender_id = self.sender_id,
                    generation,
                    action,
                    receiver_count,
                    "peer control broadcast sent"
                );
            }
            Err(error) => {
                tracing::debug!(
                    sender_id = self.sender_id,
                    generation,
                    action,
                    error = %error,
                    "peer control broadcast dropped"
                );
            }
        }
    }
}

/// Operation identifier passed to a backend in a [`BackendRequest`].
#[derive(Debug, Clone)]
pub struct BackendOp {
    /// Logical operation name (e.g. `"transcribe"`, `"generate"`).
    pub name: String,
    /// Arbitrary Payload options forwarded to the backend.
    pub options: Payload,
}

impl BackendOp {
    pub fn new(name: impl Into<String>, options: Payload) -> Self {
        Self { name: name.into(), options }
    }
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
    pub fn new(
        kind: BackendRequestKind,
        op: BackendOp,
        input: Payload,
        cancel_rx: watch::Receiver<bool>,
        broadcast_seq: Option<u64>,
        reply_tx: oneshot::Sender<BackendReply>,
    ) -> Self {
        Self { kind, op, input, cancel_rx, broadcast_seq, reply_tx }
    }

    pub fn inference(
        op: BackendOp,
        input: Payload,
        cancel_rx: watch::Receiver<bool>,
        reply_tx: oneshot::Sender<BackendReply>,
    ) -> Self {
        Self::new(BackendRequestKind::Inference, op, input, cancel_rx, None, reply_tx)
    }

    pub fn management(
        event: ManagementEvent,
        op_name: impl Into<String>,
        input: Payload,
        broadcast_seq: u64,
        reply_tx: oneshot::Sender<BackendReply>,
    ) -> Self {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        drop(cancel_tx);

        Self::new(
            BackendRequestKind::Management(event),
            BackendOp::new(op_name, Payload::default()),
            input,
            cancel_rx,
            Some(broadcast_seq),
            reply_tx,
        )
    }

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
    /// Management operation completed successfully without a payload body.
    Ack,
    /// A single complete output payload (non-streaming).
    Value(Payload),
    /// A streaming output handle (terminal stage only).
    Stream(StreamHandle),
    /// The backend encountered an error.
    Error(String),
}

impl BackendReply {
    pub const fn ack() -> Self {
        Self::Ack
    }

    pub fn value(payload: Payload) -> Self {
        Self::Value(payload)
    }

    pub fn stream(handle: StreamHandle) -> Self {
        Self::Stream(handle)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tokio::sync::{broadcast, oneshot, watch};

    use crate::base::types::Payload;

    use super::{
        BackendOp, BackendReply, BackendRequest, BackendRequestKind, DeploymentSnapshot,
        DriverRequestKind, ManagementEvent, PeerControlBus, PeerWorkerCommand,
        PeerWorkerCommandKind, RequestRoute, RuntimeControlSignal, SyncMessage, WorkerCommand,
    };
    use std::str::FromStr;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct ModelConfig {
        model: String,
    }

    #[test]
    fn request_route_string_mapping_is_lossless() {
        for route in [
            RequestRoute::LoadModel,
            RequestRoute::UnloadModel,
            RequestRoute::Inference,
            RequestRoute::InferenceStream,
            RequestRoute::InferenceImage,
        ] {
            assert_eq!(RequestRoute::from_str(route.as_str()), Ok(route));
        }
    }

    #[test]
    fn request_route_rejects_unknown_ops() {
        let error = RequestRoute::from_str("unknown.op").expect_err("unknown op");

        assert_eq!(error, "unknown backend op: unknown.op");
    }

    #[test]
    fn deployment_snapshot_decodes_typed_and_json_model_configs() {
        let typed = DeploymentSnapshot::with_model(
            7,
            Payload::typed(ModelConfig { model: "typed".to_owned() }),
        );
        let json =
            DeploymentSnapshot::with_model(8, Payload::json(serde_json::json!({"model": "json"})));

        assert_eq!(typed.generation, 7);
        assert_eq!(
            typed.typed_model_config::<ModelConfig>().expect("typed model"),
            ModelConfig { model: "typed".to_owned() }
        );
        assert_eq!(
            json.typed_model_config::<ModelConfig>().expect("json model"),
            ModelConfig { model: "json".to_owned() }
        );

        let missing = DeploymentSnapshot::default()
            .typed_model_config::<ModelConfig>()
            .expect_err("missing model");
        assert_eq!(missing, "deployment snapshot missing model config");
    }

    #[test]
    fn sync_and_peer_worker_command_metadata_are_consistent() {
        let deployment = DeploymentSnapshot::with_model(11, Payload::from("model"));
        let deployment_sync = SyncMessage::Deployment(deployment.clone());
        let generation_sync = SyncMessage::Generation { generation: 12 };

        assert_eq!(deployment_sync.generation(), 11);
        assert_eq!(generation_sync.generation(), 12);
        assert!(deployment_sync.deployment().is_some());
        assert!(generation_sync.deployment().is_none());

        let load = PeerWorkerCommand::from_kind(
            PeerWorkerCommandKind::LoadModel,
            deployment_sync.clone(),
            3,
        );
        let unload = PeerWorkerCommandKind::Unload.into_command(generation_sync, 4);

        assert_eq!(load.kind(), PeerWorkerCommandKind::LoadModel);
        assert_eq!(load.sender_id(), 3);
        assert_eq!(load.seq_id(), 11);
        assert_eq!(load.deployment().expect("deployment").generation, deployment.generation);
        assert_eq!(unload.kind(), PeerWorkerCommandKind::Unload);
        assert_eq!(unload.sender_id(), 4);
        assert_eq!(unload.seq_id(), 12);
        assert!(unload.deployment().is_none());
    }

    #[test]
    fn backend_request_builders_expose_typed_driver_views() {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (reply_tx, _reply_rx) = oneshot::channel();
        let inference = BackendRequest::inference(
            BackendOp::new("inference.stream", Payload::json(serde_json::json!({"top_p": 1}))),
            Payload::from("prompt"),
            cancel_rx,
            reply_tx,
        );
        drop(cancel_tx);

        assert_eq!(inference.kind, BackendRequestKind::Inference);
        assert_eq!(inference.route(), Ok(RequestRoute::InferenceStream));
        let DriverRequestKind::Inference(invocation) =
            inference.driver_kind().expect("inference driver kind")
        else {
            panic!("expected inference driver kind");
        };
        assert_eq!(invocation.route, RequestRoute::InferenceStream);
        assert_eq!(invocation.options.to_serde_value(), serde_json::json!({"top_p": 1}));

        let (reply_tx, _reply_rx) = oneshot::channel();
        let management = BackendRequest::management(
            ManagementEvent::LoadModel,
            "model.load",
            Payload::typed(ModelConfig { model: "load".to_owned() }),
            99,
            reply_tx,
        );

        assert_eq!(management.kind, BackendRequestKind::Management(ManagementEvent::LoadModel));
        assert_eq!(management.broadcast_seq, Some(99));
        assert!(!*management.cancel_rx.borrow());
        let DriverRequestKind::Management { event } =
            management.driver_kind().expect("management driver kind")
        else {
            panic!("expected management driver kind");
        };
        assert_eq!(event, ManagementEvent::LoadModel);
    }

    #[test]
    fn backend_request_reports_unknown_route_for_driver_views() {
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let (reply_tx, _reply_rx) = oneshot::channel();
        let request = BackendRequest::inference(
            BackendOp::new("unknown", Payload::default()),
            Payload::default(),
            cancel_rx,
            reply_tx,
        );

        assert!(request.route().expect_err("unknown route").contains("unknown backend op"));
        assert!(request.driver_kind().expect_err("unknown route").contains("unknown backend op"));
        assert!(request.invocation().expect_err("unknown route").contains("unknown backend op"));
    }

    #[tokio::test]
    async fn backend_reply_constructors_wrap_payloads_and_streams() {
        assert!(matches!(BackendReply::ack(), BackendReply::Ack));
        assert!(
            matches!(BackendReply::error("bad"), BackendReply::Error(message) if message == "bad")
        );
        assert!(matches!(BackendReply::value(Payload::from("ok")), BackendReply::Value(_)));

        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        assert!(matches!(BackendReply::stream(rx), BackendReply::Stream(_)));
    }

    #[tokio::test]
    async fn peer_control_bus_broadcasts_peer_commands_when_receivers_exist() {
        let (tx, mut rx) = broadcast::channel(4);
        let bus = PeerControlBus::new(tx, 42);

        assert_eq!(bus.sender_id(), 42);
        bus.broadcast_typed_model_loaded(5, ModelConfig { model: "loaded".to_owned() });
        bus.broadcast_model_unloaded(6);

        let first = rx.recv().await.expect("load command");
        let second = rx.recv().await.expect("unload command");

        let WorkerCommand::Peer(first) = first else {
            panic!("expected peer command");
        };
        let WorkerCommand::Peer(second) = second else {
            panic!("expected peer command");
        };

        assert_eq!(first.kind(), PeerWorkerCommandKind::LoadModel);
        assert_eq!(first.sender_id(), 42);
        assert_eq!(first.seq_id(), 5);
        assert_eq!(
            first
                .deployment()
                .expect("deployment")
                .typed_model_config::<ModelConfig>()
                .expect("model"),
            ModelConfig { model: "loaded".to_owned() }
        );
        assert_eq!(second.kind(), PeerWorkerCommandKind::Unload);
        assert_eq!(second.seq_id(), 6);
    }

    #[test]
    fn runtime_control_signal_debug_contains_variant_names() {
        let load = RuntimeControlSignal::GlobalLoad { op_id: 1, payload: Payload::default() };
        let unload = RuntimeControlSignal::GlobalUnload { op_id: 2 };

        assert!(format!("{load:?}").contains("GlobalLoad"));
        assert!(format!("{unload:?}").contains("GlobalUnload"));
    }
}
