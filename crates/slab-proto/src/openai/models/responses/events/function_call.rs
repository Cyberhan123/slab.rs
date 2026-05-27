use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum FuncArgsDeltaType {
    #[serde(rename = "response.function_call_arguments.delta")]
    #[default]
    ResponseFunctionCallArgumentsDelta,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFunctionCallArgumentsDeltaEvent {
    /// The type of the event. Always `response.function_call_arguments.delta`.
    #[serde(rename = "type")]
    pub r#type: FuncArgsDeltaType,
    /// The ID of the output item that the function-call arguments delta is added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the function-call arguments delta is added to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The function-call arguments delta that is added.
    #[serde(rename = "delta")]
    pub delta: String,
}

impl ResponseFunctionCallArgumentsDeltaEvent {
    /// Emitted when there is a partial function-call arguments delta.
    pub fn new(
        r#type: FuncArgsDeltaType,
        item_id: String,
        output_index: i32,
        sequence_number: i32,
        delta: String,
    ) -> ResponseFunctionCallArgumentsDeltaEvent {
        ResponseFunctionCallArgumentsDeltaEvent {
            r#type,
            item_id,
            output_index,
            sequence_number,
            delta,
        }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum FuncArgsDoneType {
    #[serde(rename = "response.function_call_arguments.done")]
    #[default]
    ResponseFunctionCallArgumentsDone,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFunctionCallArgumentsDoneEvent {
    #[serde(rename = "type")]
    pub r#type: FuncArgsDoneType,
    /// The ID of the item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The name of the function that was called.
    #[serde(rename = "name")]
    pub name: String,
    /// The index of the output item.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The function-call arguments.
    #[serde(rename = "arguments")]
    pub arguments: String,
}

impl ResponseFunctionCallArgumentsDoneEvent {
    /// Emitted when function-call arguments are finalized.
    pub fn new(
        r#type: FuncArgsDoneType,
        item_id: String,
        name: String,
        output_index: i32,
        sequence_number: i32,
        arguments: String,
    ) -> ResponseFunctionCallArgumentsDoneEvent {
        ResponseFunctionCallArgumentsDoneEvent {
            r#type,
            item_id,
            name,
            output_index,
            sequence_number,
            arguments,
        }
    }
}
