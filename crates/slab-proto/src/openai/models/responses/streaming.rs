use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseStreamEvent {
    #[serde(rename = "ResponsesServerEvent")]
    ResponsesServerEvent {
        /// The sequence number of this event.
        #[serde(rename = "sequence_number")]
        sequence_number: i32,
        /// The incremental input data (delta) for the custom tool call.
        #[serde(rename = "delta")]
        delta: String,
        /// The index of the output this event applies to.
        #[serde(rename = "output_index")]
        output_index: i32,
        /// Unique identifier for the API item associated with this event.
        #[serde(rename = "item_id")]
        item_id: String,
        /// The error code.
        #[serde(rename = "code")]
        code: String,
        /// The full response object that is queued.
        #[serde(rename = "response")]
        response: Box<models::Response>,
        /// The index of the content part within the output item.
        #[serde(rename = "content_index")]
        content_index: i32,
        #[serde(rename = "part")]
        part: Box<models::ResponseReasoningSummaryPartDoneEventPart>,
        /// The error message.
        #[serde(rename = "message")]
        message: String,
        /// The error parameter.
        #[serde(rename = "param")]
        param: String,
        /// The name of the function that was called.
        #[serde(rename = "name")]
        name: String,
        /// A JSON string containing the finalized arguments for the MCP tool call.
        #[serde(rename = "arguments")]
        arguments: String,
        /// The output item that was marked done.
        #[serde(rename = "item")]
        item: Box<models::OutputItem>,
        /// The index of the summary part within the reasoning summary.
        #[serde(rename = "summary_index")]
        summary_index: i32,
        /// The text content that is finalized.
        #[serde(rename = "text")]
        text: String,
        /// The refusal text that is finalized.
        #[serde(rename = "refusal")]
        refusal: String,
        /// The log probabilities of the tokens in the delta.
        #[serde(rename = "logprobs")]
        logprobs: Vec<models::ResponseLogProb>,
        /// 0-based index for the partial image (backend is 1-based, but this is 0-based for the user).
        #[serde(rename = "partial_image_index")]
        partial_image_index: i32,
        /// Base64-encoded partial image data, suitable for rendering as an image.
        #[serde(rename = "partial_image_b64")]
        partial_image_b64: String,
        /// The index of the annotation within the content part.
        #[serde(rename = "annotation_index")]
        annotation_index: i32,
        /// The annotation object being added. (See annotation schema for details.)
        #[serde(rename = "annotation")]
        annotation: serde_json::Value,
        /// The complete input data for the custom tool call.
        #[serde(rename = "input")]
        input: String,
    },
}

impl Default for ResponseStreamEvent {
    fn default() -> Self {
        Self::ResponsesServerEvent {
            sequence_number: Default::default(),
            delta: Default::default(),
            output_index: Default::default(),
            item_id: Default::default(),
            code: Default::default(),
            response: Default::default(),
            content_index: Default::default(),
            part: Default::default(),
            message: Default::default(),
            param: Default::default(),
            name: Default::default(),
            arguments: Default::default(),
            item: Default::default(),
            summary_index: Default::default(),
            text: Default::default(),
            refusal: Default::default(),
            logprobs: Default::default(),
            partial_image_index: Default::default(),
            partial_image_b64: Default::default(),
            annotation_index: Default::default(),
            annotation: Default::default(),
            input: Default::default(),
        }
    }
}

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
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum DoneEventEvent {
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
///
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ErrorEventEvent {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Data {
    #[serde(rename = "[DONE]")]
    #[default]
    LeftSquareBracketDoneRightSquareBracket,
}

