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
