use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseRefusalDeltaEvent {
    /// The type of the event. Always `response.refusal.delta`.
    #[serde(rename = "type")]
    pub r#type: RefusalDeltaType,
    /// The ID of the output item that the refusal text is added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the refusal text is added to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that the refusal text is added to.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The refusal text that is added.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseRefusalDeltaEvent {
    /// Emitted when there is a partial refusal text.
    pub fn new(
        r#type: RefusalDeltaType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
        sequence_number: i32,
    ) -> ResponseRefusalDeltaEvent {
        ResponseRefusalDeltaEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            delta,
            sequence_number,
        }
    }
}
pub mod refusal_delta_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.refusal.delta`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.refusal.delta")]
        #[default]
        ResponseRefusalDelta,
    }
    
}
pub use refusal_delta_type::Type as RefusalDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseRefusalDoneEvent {
    /// The type of the event. Always `response.refusal.done`.
    #[serde(rename = "type")]
    pub r#type: RefusalDoneType,
    /// The ID of the output item that the refusal text is finalized.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the refusal text is finalized.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that the refusal text is finalized.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The refusal text that is finalized.
    #[serde(rename = "refusal")]
    pub refusal: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseRefusalDoneEvent {
    /// Emitted when refusal text is finalized.
    pub fn new(
        r#type: RefusalDoneType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        refusal: String,
        sequence_number: i32,
    ) -> ResponseRefusalDoneEvent {
        ResponseRefusalDoneEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            refusal,
            sequence_number,
        }
    }
}
pub mod refusal_done_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.refusal.done`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.refusal.done")]
        #[default]
        ResponseRefusalDone,
    }
    
}
pub use refusal_done_type::Type as RefusalDoneType;
