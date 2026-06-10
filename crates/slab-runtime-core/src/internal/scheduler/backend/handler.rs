use std::fmt;
use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::base::types::Payload;

use super::protocol::{
    BackendReply, BackendRequest, PeerWorkerCommand, RuntimeControlSignal, StreamHandle,
};

/// Typed input extracted by macro-generated worker handlers.
///
/// `#[on_event(...)]` reads from [`BackendRequest::input`], while typed control
/// handlers decode from the control payload carried by the matched signal.
#[derive(Debug, Clone)]
pub struct Input<T>(pub T);

/// Typed options extracted from [`BackendRequest::op.options`] for event handlers.
#[derive(Debug, Clone)]
pub struct Options<T>(pub T);

/// Cancellation receiver extracted from [`BackendRequest::cancel_rx`] for event handlers.
#[derive(Debug, Clone)]
pub struct CancelRx(pub tokio::sync::watch::Receiver<bool>);

/// Broadcast sequence extracted from event or peer-control metadata.
#[derive(Debug, Clone, Copy, Default)]
pub struct BroadcastSeq(pub u64);

/// Runtime control operation id extracted from [`RuntimeControlSignal`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ControlOpId(pub u64);

/// Structured JSON response wrapper for typed event handlers.
///
/// Control handlers do not emit transport replies, so they always use `()` or
/// `Result<(), E>` instead of `Json<T>`.
#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

/// Structured typed-payload response wrapper for typed event handlers.
///
/// Control handlers do not emit transport replies, so they always use `()` or
/// `Result<(), E>` instead of `Typed<T>`.
#[derive(Debug, Clone)]
pub struct Typed<T>(pub T);

/// Internal error used by macro-generated backend handler glue.
///
/// Backend handlers themselves may return any error type implementing
/// `Display`; this wrapper is reserved for extractor and reply-adapter errors
/// inside `slab-runtime-core`.
#[derive(Debug, Clone, Error)]
#[error("{0}")]
pub struct BackendHandlerError(String);

impl BackendHandlerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<String> for BackendHandlerError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BackendHandlerError {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<BackendHandlerError> for String {
    fn from(value: BackendHandlerError) -> Self {
        value.0
    }
}

/// Convert a typed handler success value into a backend reply.
pub trait IntoBackendReply {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError>;
}

impl IntoBackendReply for () {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::ack())
    }
}

impl IntoBackendReply for Payload {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(self))
    }
}

impl IntoBackendReply for StreamHandle {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::stream(self))
    }
}

impl IntoBackendReply for String {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::text(self)))
    }
}

impl IntoBackendReply for Arc<str> {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::text(self)))
    }
}

impl IntoBackendReply for Vec<u8> {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::from(self)))
    }
}

impl IntoBackendReply for Arc<[u8]> {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::Bytes(self)))
    }
}

impl IntoBackendReply for Vec<f32> {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::from(self)))
    }
}

impl IntoBackendReply for Arc<[f32]> {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::F32(self)))
    }
}

impl IntoBackendReply for serde_json::Value {
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::json(self)))
    }
}

impl<T> IntoBackendReply for Json<T>
where
    T: Serialize,
{
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        serde_json::to_value(self.0).map(Payload::json).map(BackendReply::value).map_err(|error| {
            BackendHandlerError::new(format!("failed to serialize backend json response: {error}"))
        })
    }
}

impl<T> IntoBackendReply for Typed<T>
where
    T: Send + Sync + 'static,
{
    fn into_backend_reply(self) -> Result<BackendReply, BackendHandlerError> {
        Ok(BackendReply::value(Payload::typed(self.0)))
    }
}

/// Collapse a typed event handler result into the transport reply expected by the runner.
///
/// Only `#[on_event(...)]` handlers flow through this adapter because they own a
/// request reply channel. Typed control handlers are fire-and-forget and are
/// limited to `()` / `Result<(), E>`.
pub fn backend_reply_from_event_result<T, E>(result: Result<T, E>) -> BackendReply
where
    T: IntoBackendReply,
    E: fmt::Display,
{
    match result {
        Ok(value) => match value.into_backend_reply() {
            Ok(reply) => reply,
            Err(error) => BackendReply::error(error.to_string()),
        },
        Err(error) => BackendReply::error(error.to_string()),
    }
}

pub fn log_runtime_control_extractor_failure(
    backend: &'static str,
    route: &'static str,
    signal: &RuntimeControlSignal,
    error: impl fmt::Display,
) {
    let (control_kind, op_id) = match signal {
        RuntimeControlSignal::GlobalLoad { op_id, .. } => ("GlobalLoad", Some(*op_id)),
        RuntimeControlSignal::GlobalUnload { op_id } => ("GlobalUnload", Some(*op_id)),
    };
    tracing::error!(
        backend,
        route,
        control_kind,
        op_id,
        error = %error,
        "backend runtime control extractor failed"
    );
}

pub fn log_runtime_control_handler_failure(
    backend: &'static str,
    route: &'static str,
    signal: &RuntimeControlSignal,
    error: impl fmt::Display,
) {
    let (control_kind, op_id) = match signal {
        RuntimeControlSignal::GlobalLoad { op_id, .. } => ("GlobalLoad", Some(*op_id)),
        RuntimeControlSignal::GlobalUnload { op_id } => ("GlobalUnload", Some(*op_id)),
    };
    tracing::error!(
        backend,
        route,
        control_kind,
        op_id,
        error = %error,
        "backend runtime control handler failed"
    );
}

pub fn log_peer_control_extractor_failure(
    backend: &'static str,
    route: &'static str,
    cmd: &PeerWorkerCommand,
    error: impl fmt::Display,
) {
    tracing::error!(
        backend,
        route,
        control_kind = cmd.kind().as_str(),
        seq_id = cmd.seq_id(),
        sender_id = cmd.sender_id(),
        error = %error,
        "backend peer control extractor failed"
    );
}

pub fn log_peer_control_handler_failure(
    backend: &'static str,
    route: &'static str,
    cmd: &PeerWorkerCommand,
    error: impl fmt::Display,
) {
    tracing::error!(
        backend,
        route,
        control_kind = cmd.kind().as_str(),
        seq_id = cmd.seq_id(),
        sender_id = cmd.sender_id(),
        error = %error,
        "backend peer control handler failed"
    );
}

pub fn log_lagged_control_handler_failure(
    backend: &'static str,
    route: &'static str,
    error: impl fmt::Display,
) {
    tracing::error!(
        backend,
        route,
        control_kind = "LaggedControl",
        error = %error,
        "backend lagged control handler failed"
    );
}

pub fn extract_event_text(req: &BackendRequest) -> Result<String, BackendHandlerError> {
    req.input
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|error| BackendHandlerError::new(format!("invalid event text input: {error}")))
}

pub fn extract_event_payload(req: &BackendRequest) -> Result<Payload, BackendHandlerError> {
    Ok(req.input.clone())
}

pub fn extract_event_input<T>(req: &BackendRequest) -> Result<Input<T>, BackendHandlerError>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    req.input
        .to_typed::<T>()
        .map(Input)
        .map_err(|error| BackendHandlerError::new(format!("invalid event input: {error}")))
}

pub fn extract_event_options<T>(req: &BackendRequest) -> Result<Options<T>, BackendHandlerError>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    req.op
        .options
        .to_typed::<T>()
        .map(Options)
        .map_err(|error| BackendHandlerError::new(format!("invalid event options: {error}")))
}

pub fn extract_event_cancel_rx(req: &BackendRequest) -> Result<CancelRx, BackendHandlerError> {
    Ok(CancelRx(req.cancel_rx.clone()))
}

pub fn extract_event_broadcast_seq(
    req: &BackendRequest,
) -> Result<BroadcastSeq, BackendHandlerError> {
    Ok(BroadcastSeq(req.broadcast_seq.unwrap_or(0)))
}

/// Extract typed metadata from a matched runtime control signal.
pub fn extract_runtime_control_op_id(
    signal: &RuntimeControlSignal,
) -> Result<ControlOpId, BackendHandlerError> {
    let op_id = match signal {
        RuntimeControlSignal::GlobalLoad { op_id, .. }
        | RuntimeControlSignal::GlobalUnload { op_id } => *op_id,
    };
    Ok(ControlOpId(op_id))
}

pub fn extract_runtime_control_payload(
    signal: &RuntimeControlSignal,
) -> Result<Payload, BackendHandlerError> {
    match signal {
        RuntimeControlSignal::GlobalLoad { payload, .. } => Ok(payload.clone()),
        RuntimeControlSignal::GlobalUnload { .. } => {
            Err(BackendHandlerError::new("runtime control payload unavailable for GlobalUnload"))
        }
    }
}

pub fn extract_runtime_control_input<T>(
    signal: &RuntimeControlSignal,
) -> Result<Input<T>, BackendHandlerError>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    extract_runtime_control_payload(signal)?.to_typed::<T>().map(Input).map_err(|error| {
        BackendHandlerError::new(format!("invalid runtime control input: {error}"))
    })
}

/// Extract typed metadata from a matched peer control command.
pub fn extract_peer_control_payload(
    cmd: &PeerWorkerCommand,
) -> Result<Payload, BackendHandlerError> {
    cmd.deployment().and_then(|snapshot| snapshot.model.clone()).ok_or_else(|| {
        BackendHandlerError::new("peer control payload unavailable for this command")
    })
}

pub fn extract_peer_control_input<T>(
    cmd: &PeerWorkerCommand,
) -> Result<Input<T>, BackendHandlerError>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    extract_peer_control_payload(cmd)?
        .to_typed::<T>()
        .map(Input)
        .map_err(|error| BackendHandlerError::new(format!("invalid peer control input: {error}")))
}

pub fn extract_peer_control_broadcast_seq(
    cmd: &PeerWorkerCommand,
) -> Result<BroadcastSeq, BackendHandlerError> {
    Ok(BroadcastSeq(cmd.seq_id()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde::{Deserialize, Serialize, Serializer};
    use tokio::sync::{oneshot, watch};

    use crate::base::types::{Payload, StreamChunk};

    use super::super::protocol::{
        BackendOp, BackendReply, BackendRequest, BackendRequestKind, PeerWorkerCommand,
        RuntimeControlSignal, SyncMessage,
    };
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct Sample {
        value: String,
    }

    struct BrokenSerialize;

    impl Serialize for BrokenSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom("broken serialize"))
        }
    }

    fn request(input: Payload, options: Payload, broadcast_seq: Option<u64>) -> BackendRequest {
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let (reply_tx, _reply_rx) = oneshot::channel();
        BackendRequest::new(
            BackendRequestKind::Inference,
            BackendOp::new("inference", options),
            input,
            cancel_rx,
            broadcast_seq,
            reply_tx,
        )
    }

    #[test]
    fn into_backend_reply_maps_common_success_values() {
        assert!(matches!(().into_backend_reply().expect("unit"), BackendReply::Ack));
        assert!(matches!(
            Payload::from("payload").into_backend_reply().expect("payload"),
            BackendReply::Value(Payload::Text(_))
        ));
        assert!(matches!(
            "text".to_owned().into_backend_reply().expect("string"),
            BackendReply::Value(Payload::Text(_))
        ));
        assert!(matches!(
            Arc::<str>::from("arc text").into_backend_reply().expect("arc str"),
            BackendReply::Value(Payload::Text(_))
        ));
        assert!(matches!(
            vec![1_u8, 2].into_backend_reply().expect("bytes"),
            BackendReply::Value(Payload::Bytes(_))
        ));
        assert!(matches!(
            Arc::<[u8]>::from([1_u8, 2]).into_backend_reply().expect("arc bytes"),
            BackendReply::Value(Payload::Bytes(_))
        ));
        assert!(matches!(
            vec![1.0_f32].into_backend_reply().expect("floats"),
            BackendReply::Value(Payload::F32(_))
        ));
        assert!(matches!(
            Arc::<[f32]>::from([1.0_f32]).into_backend_reply().expect("arc floats"),
            BackendReply::Value(Payload::F32(_))
        ));
        assert!(matches!(
            serde_json::json!({"ok": true}).into_backend_reply().expect("json"),
            BackendReply::Value(Payload::Json(_))
        ));
        assert!(matches!(
            Json(Sample { value: "json".to_owned() }).into_backend_reply().expect("json wrapper"),
            BackendReply::Value(Payload::Json(_))
        ));
        assert!(matches!(
            Typed(Sample { value: "typed".to_owned() })
                .into_backend_reply()
                .expect("typed wrapper"),
            BackendReply::Value(Payload::Typed(_))
        ));

        let (_tx, rx) = tokio::sync::mpsc::channel::<StreamChunk>(1);
        assert!(matches!(rx.into_backend_reply().expect("stream"), BackendReply::Stream(_)));
    }

    #[test]
    fn backend_reply_from_event_result_preserves_handler_and_adapter_errors() {
        assert!(matches!(
            backend_reply_from_event_result::<_, &str>(Ok("ok".to_owned())),
            BackendReply::Value(Payload::Text(_))
        ));
        assert!(matches!(
            backend_reply_from_event_result::<String, _>(Err("handler failed")),
            BackendReply::Error(message) if message == "handler failed"
        ));
        assert!(matches!(
            backend_reply_from_event_result::<_, &str>(Ok(Json(BrokenSerialize))),
            BackendReply::Error(message) if message.contains("failed to serialize backend json response")
        ));
    }

    #[test]
    fn event_extractors_decode_text_payload_options_cancel_and_sequence() {
        let req = request(
            Payload::json(serde_json::json!({"value": "input"})),
            Payload::typed(Sample { value: "options".to_owned() }),
            Some(17),
        );

        assert!(matches!(extract_event_payload(&req).expect("payload"), Payload::Json(_)));
        assert_eq!(
            extract_event_input::<Sample>(&req).expect("input").0,
            Sample { value: "input".to_owned() }
        );
        assert_eq!(
            extract_event_options::<Sample>(&req).expect("options").0,
            Sample { value: "options".to_owned() }
        );
        assert_eq!(extract_event_broadcast_seq(&req).expect("seq").0, 17);
        assert!(!*extract_event_cancel_rx(&req).expect("cancel").0.borrow());

        let text_req = request(Payload::from("hello"), Payload::default(), None);
        assert_eq!(extract_event_text(&text_req).expect("text"), "hello");
        assert_eq!(extract_event_broadcast_seq(&text_req).expect("default seq").0, 0);
    }

    #[test]
    fn event_extractors_report_type_errors_with_context() {
        let req = request(Payload::from(vec![1_u8]), Payload::from("bad options"), None);

        let text_error = extract_event_text(&req).expect_err("bytes are not text");
        assert!(text_error.to_string().contains("invalid event text input"));

        let input_error = extract_event_input::<Sample>(&req).expect_err("bytes are not typed");
        assert!(input_error.to_string().contains("invalid event input"));

        let options_error =
            extract_event_options::<Sample>(&req).expect_err("text options are not typed");
        assert!(options_error.to_string().contains("invalid event options"));
    }

    #[test]
    fn runtime_control_extractors_handle_load_and_unload_boundaries() {
        let load = RuntimeControlSignal::GlobalLoad {
            op_id: 31,
            payload: Payload::json(serde_json::json!({"value": "runtime"})),
        };
        let unload = RuntimeControlSignal::GlobalUnload { op_id: 32 };

        assert_eq!(extract_runtime_control_op_id(&load).expect("load id").0, 31);
        assert_eq!(extract_runtime_control_op_id(&unload).expect("unload id").0, 32);
        assert!(matches!(
            extract_runtime_control_payload(&load).expect("payload"),
            Payload::Json(_)
        ));
        assert_eq!(
            extract_runtime_control_input::<Sample>(&load).expect("typed load").0,
            Sample { value: "runtime".to_owned() }
        );

        let unload_error =
            extract_runtime_control_payload(&unload).expect_err("unload has no payload");
        assert!(unload_error.to_string().contains("payload unavailable"));
    }

    #[test]
    fn peer_control_extractors_handle_deployments_and_generation_only_commands() {
        let load = PeerWorkerCommand::LoadModel {
            sync: SyncMessage::Deployment(super::super::protocol::DeploymentSnapshot::with_model(
                41,
                Payload::typed(Sample { value: "peer".to_owned() }),
            )),
            sender_id: 2,
        };
        let unload = PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 42 },
            sender_id: 3,
        };

        assert!(matches!(extract_peer_control_payload(&load).expect("payload"), Payload::Typed(_)));
        assert_eq!(
            extract_peer_control_input::<Sample>(&load).expect("typed peer").0,
            Sample { value: "peer".to_owned() }
        );
        assert_eq!(extract_peer_control_broadcast_seq(&load).expect("seq").0, 41);
        assert_eq!(extract_peer_control_broadcast_seq(&unload).expect("unload seq").0, 42);

        let error = extract_peer_control_payload(&unload).expect_err("generation has no payload");
        assert!(error.to_string().contains("payload unavailable"));
    }

    #[test]
    fn logging_helpers_accept_all_control_shapes() {
        let load = RuntimeControlSignal::GlobalLoad { op_id: 1, payload: Payload::default() };
        let unload = RuntimeControlSignal::GlobalUnload { op_id: 2 };
        let peer = PeerWorkerCommand::Unload {
            sync: SyncMessage::Generation { generation: 3 },
            sender_id: 4,
        };

        log_runtime_control_extractor_failure("test", "load", &load, "bad load");
        log_runtime_control_handler_failure("test", "unload", &unload, "bad unload");
        log_peer_control_extractor_failure("test", "peer", &peer, "bad peer");
        log_peer_control_handler_failure("test", "peer", &peer, "bad peer");
        log_lagged_control_handler_failure("test", "lagged", "lagged");
    }

    #[test]
    fn backend_handler_error_round_trips_to_string() {
        let error = BackendHandlerError::from("message");
        let value: String = error.clone().into();

        assert_eq!(value, "message");
        assert_eq!(error.to_string(), "message");
    }
}
