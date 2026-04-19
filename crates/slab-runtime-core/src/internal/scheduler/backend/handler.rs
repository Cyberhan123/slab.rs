use std::fmt;
use std::sync::Arc;

use serde::{Serialize, de::DeserializeOwned};

use crate::base::types::Payload;

use super::protocol::{BackendReply, BackendRequest, StreamHandle};

/// Typed input extracted from [`BackendRequest::input`].
#[derive(Debug, Clone)]
pub struct Input<T>(pub T);

/// Typed options extracted from [`BackendRequest::op.options`].
#[derive(Debug, Clone)]
pub struct Options<T>(pub T);

/// Cancellation receiver extracted from [`BackendRequest::cancel_rx`].
#[derive(Debug, Clone)]
pub struct CancelRx(pub tokio::sync::watch::Receiver<bool>);

/// Broadcast sequence extracted from [`BackendRequest::broadcast_seq`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BroadcastSeq(pub u64);

/// Structured JSON response wrapper for typed event handlers.
#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

/// Structured typed-payload response wrapper for typed event handlers.
#[derive(Debug, Clone)]
pub struct Typed<T>(pub T);

/// Convert a typed handler success value into a backend reply.
pub trait IntoBackendReply {
    fn into_backend_reply(self) -> Result<BackendReply, String>;
}

impl IntoBackendReply for () {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::ack())
    }
}

impl IntoBackendReply for Payload {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(self))
    }
}

impl IntoBackendReply for StreamHandle {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::stream(self))
    }
}

impl IntoBackendReply for String {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::text(self)))
    }
}

impl IntoBackendReply for Arc<str> {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::text(self)))
    }
}

impl IntoBackendReply for Vec<u8> {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::from(self)))
    }
}

impl IntoBackendReply for Arc<[u8]> {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::Bytes(self)))
    }
}

impl IntoBackendReply for Vec<f32> {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::from(self)))
    }
}

impl IntoBackendReply for Arc<[f32]> {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::F32(self)))
    }
}

impl IntoBackendReply for serde_json::Value {
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::json(self)))
    }
}

impl<T> IntoBackendReply for Json<T>
where
    T: Serialize,
{
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        serde_json::to_value(self.0)
            .map(Payload::json)
            .map(BackendReply::value)
            .map_err(|error| format!("failed to serialize backend json response: {error}"))
    }
}

impl<T> IntoBackendReply for Typed<T>
where
    T: Send + Sync + 'static,
{
    fn into_backend_reply(self) -> Result<BackendReply, String> {
        Ok(BackendReply::value(Payload::typed(self.0)))
    }
}

/// Collapse a typed event handler result into the transport reply expected by the runner.
pub fn backend_reply_from_event_result<T, E>(result: Result<T, E>) -> BackendReply
where
    T: IntoBackendReply,
    E: fmt::Display,
{
    match result {
        Ok(value) => match value.into_backend_reply() {
            Ok(reply) => reply,
            Err(message) => BackendReply::error(message),
        },
        Err(error) => BackendReply::error(error.to_string()),
    }
}

pub fn extract_event_text(req: &BackendRequest) -> Result<String, String> {
    req.input
        .to_str()
        .map(ToOwned::to_owned)
        .map_err(|error| format!("invalid event text input: {error}"))
}

pub fn extract_event_payload(req: &BackendRequest) -> Result<Payload, String> {
    Ok(req.input.clone())
}

pub fn extract_event_input<T>(req: &BackendRequest) -> Result<Input<T>, String>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    req.input
        .to_typed::<T>()
        .map(Input)
        .map_err(|error| format!("invalid event input: {error}"))
}

pub fn extract_event_options<T>(req: &BackendRequest) -> Result<Options<T>, String>
where
    T: DeserializeOwned + Clone + Send + Sync + 'static,
{
    req.op
        .options
        .to_typed::<T>()
        .map(Options)
        .map_err(|error| format!("invalid event options: {error}"))
}

pub fn extract_event_cancel_rx(req: &BackendRequest) -> Result<CancelRx, String> {
    Ok(CancelRx(req.cancel_rx.clone()))
}

pub fn extract_event_broadcast_seq(req: &BackendRequest) -> Result<BroadcastSeq, String> {
    Ok(BroadcastSeq(req.broadcast_seq.unwrap_or(0)))
}
