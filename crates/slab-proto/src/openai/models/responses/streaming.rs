use crate::openai::models;
use serde::{Deserialize, Serialize};

pub type ResponseStreamEvent = super::ResponsesServerEvent;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct DoneEvent {
    #[serde(rename = "event")]
    pub event: DoneEventEvent,
    #[serde(rename = "data")]
    pub data: Data,
}

impl DoneEvent {
    /// Occurs when a stream ends.
    pub fn new(event: DoneEventEvent, data: Data) -> DoneEvent {
        DoneEvent { event, data }
    }
}
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum DoneEventEvent {
    #[serde(rename = "done")]
    #[default]
    Done,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorEvent {
    #[serde(rename = "event")]
    pub event: ErrorEventEvent,
    #[serde(rename = "data")]
    pub data: Box<models::Error>,
}

impl ErrorEvent {
    /// Occurs when an [error](/docs/guides/error-codes#api-errors) occurs. This can happen due to an internal server error or a timeout.
    pub fn new(event: ErrorEventEvent, data: models::Error) -> ErrorEvent {
        ErrorEvent { event, data: Box::new(data) }
    }
}
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ErrorEventEvent {
    #[serde(rename = "error")]
    #[default]
    Error,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseStreamOptions {
    /// When true, stream obfuscation will be enabled. Stream obfuscation adds random characters to an `obfuscation` field on streaming delta events to normalize payload sizes as a mitigation to certain side-channel attacks. These obfuscation fields are included by default, but add a small amount of overhead to the data stream. You can set `include_obfuscation` to false to optimize for bandwidth if you trust the network links between your application and the OpenAI API.
    #[serde(rename = "include_obfuscation", skip_serializing_if = "Option::is_none")]
    pub include_obfuscation: Option<bool>,
}

impl ResponseStreamOptions {
    /// Options for streaming responses. Only set this when you set `stream: true`.
    pub fn new() -> ResponseStreamOptions {
        ResponseStreamOptions { include_obfuscation: None }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum Data {
    #[serde(rename = "[DONE]")]
    #[default]
    LeftSquareBracketDoneRightSquareBracket,
}
