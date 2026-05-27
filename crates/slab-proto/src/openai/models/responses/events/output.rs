use crate::models;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum OutputItemAddedType {
    #[serde(rename = "response.output_item.added")]
    #[default]
    ResponseOutputItemAdded,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseOutputItemAddedEvent {
    /// The type of the event. Always `response.output_item.added`.
    #[serde(rename = "type")]
    pub r#type: OutputItemAddedType,
    /// The index of the output item that was added.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The output item that was added.
    #[serde(rename = "item")]
    pub item: Box<models::OutputItem>,
}

impl ResponseOutputItemAddedEvent {
    /// Emitted when a new output item is added.
    pub fn new(
        r#type: OutputItemAddedType,
        output_index: i32,
        sequence_number: i32,
        item: models::OutputItem,
    ) -> ResponseOutputItemAddedEvent {
        ResponseOutputItemAddedEvent { r#type, output_index, sequence_number, item: Box::new(item) }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum OutputItemDoneType {
    #[serde(rename = "response.output_item.done")]
    #[default]
    ResponseOutputItemDone,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseOutputItemDoneEvent {
    /// The type of the event. Always `response.output_item.done`.
    #[serde(rename = "type")]
    pub r#type: OutputItemDoneType,
    /// The index of the output item that was marked done.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The output item that was marked done.
    #[serde(rename = "item")]
    pub item: Box<models::OutputItem>,
}

impl ResponseOutputItemDoneEvent {
    /// Emitted when an output item is marked done.
    pub fn new(
        r#type: OutputItemDoneType,
        output_index: i32,
        sequence_number: i32,
        item: models::OutputItem,
    ) -> ResponseOutputItemDoneEvent {
        ResponseOutputItemDoneEvent { r#type, output_index, sequence_number, item: Box::new(item) }
    }
}
