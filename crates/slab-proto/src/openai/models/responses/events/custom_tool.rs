use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCustomToolCallInputDeltaEvent {
    /// The event type identifier.
    #[serde(rename = "type")]
    pub r#type: CustomToolCallInputDeltaType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The index of the output this delta applies to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique identifier for the API item associated with this event.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The incremental input data (delta) for the custom tool call.
    #[serde(rename = "delta")]
    pub delta: String,
}

impl ResponseCustomToolCallInputDeltaEvent {
    /// ResponseCustomToolCallInputDeltaEvent representing a delta (partial update) to the input of a custom tool call.
    pub fn new(
        r#type: CustomToolCallInputDeltaType,
        sequence_number: i32,
        output_index: i32,
        item_id: String,
        delta: String,
    ) -> ResponseCustomToolCallInputDeltaEvent {
        ResponseCustomToolCallInputDeltaEvent {
            r#type,
            sequence_number,
            output_index,
            item_id,
            delta,
        }
    }
}
pub mod custom_tool_call_input_delta_type {
    use serde::{Deserialize, Serialize};
    /// The event type identifier.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.custom_tool_call_input.delta")]
        ResponseCustomToolCallInputDelta,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCustomToolCallInputDelta
        }
    }
}
pub use custom_tool_call_input_delta_type::Type as CustomToolCallInputDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCustomToolCallInputDoneEvent {
    /// The event type identifier.
    #[serde(rename = "type")]
    pub r#type: CustomToolCallInputDoneType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The index of the output this event applies to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique identifier for the API item associated with this event.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The complete input data for the custom tool call.
    #[serde(rename = "input")]
    pub input: String,
}

impl ResponseCustomToolCallInputDoneEvent {
    /// ResponseCustomToolCallInputDoneEvent indicating that input for a custom tool call is complete.
    pub fn new(
        r#type: CustomToolCallInputDoneType,
        sequence_number: i32,
        output_index: i32,
        item_id: String,
        input: String,
    ) -> ResponseCustomToolCallInputDoneEvent {
        ResponseCustomToolCallInputDoneEvent {
            r#type,
            sequence_number,
            output_index,
            item_id,
            input,
        }
    }
}
pub mod custom_tool_call_input_done_type {
    use serde::{Deserialize, Serialize};
    /// The event type identifier.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.custom_tool_call_input.done")]
        ResponseCustomToolCallInputDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCustomToolCallInputDone
        }
    }
}
pub use custom_tool_call_input_done_type::Type as CustomToolCallInputDoneType;
