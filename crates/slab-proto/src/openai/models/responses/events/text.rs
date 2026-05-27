use crate::models;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum TextDeltaType {
    #[serde(rename = "response.output_text.delta")]
    #[default]
    ResponseOutputTextDelta,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseTextDeltaEvent {
    /// The type of the event. Always `response.output_text.delta`.
    #[serde(rename = "type")]
    pub r#type: TextDeltaType,
    /// The ID of the output item that the text delta was added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the text delta was added to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that the text delta was added to.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The text delta that was added.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number for this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The log probabilities of the tokens in the delta.
    #[serde(rename = "logprobs")]
    pub logprobs: Vec<models::ResponseLogProb>,
}

impl ResponseTextDeltaEvent {
    /// Emitted when there is an additional text delta.
    pub fn new(
        r#type: TextDeltaType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
        sequence_number: i32,
        logprobs: Vec<models::ResponseLogProb>,
    ) -> ResponseTextDeltaEvent {
        ResponseTextDeltaEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            delta,
            sequence_number,
            logprobs,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum TextDoneType {
    #[serde(rename = "response.output_text.done")]
    #[default]
    ResponseOutputTextDone,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseTextDoneEvent {
    /// The type of the event. Always `response.output_text.done`.
    #[serde(rename = "type")]
    pub r#type: TextDoneType,
    /// The ID of the output item that the text content is finalized.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the text content is finalized.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that the text content is finalized.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The text content that is finalized.
    #[serde(rename = "text")]
    pub text: String,
    /// The sequence number for this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The log probabilities of the tokens in the delta.
    #[serde(rename = "logprobs")]
    pub logprobs: Vec<models::ResponseLogProb>,
}

impl ResponseTextDoneEvent {
    /// Emitted when text content is finalized.
    pub fn new(
        r#type: TextDoneType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        text: String,
        sequence_number: i32,
        logprobs: Vec<models::ResponseLogProb>,
    ) -> ResponseTextDoneEvent {
        ResponseTextDoneEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            text,
            sequence_number,
            logprobs,
        }
    }
}
