use serde::{Deserialize, Serialize};

pub mod code_delta_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.code_interpreter_call_code.delta")]
        ResponseCodeInterpreterCallCodeDelta,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCodeInterpreterCallCodeDelta
        }
    }
}
pub use code_delta_type::Type as CodeDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCodeInterpreterCallCodeDeltaEvent {
    /// The type of the event. Always `response.code_interpreter_call_code.delta`.
    #[serde(rename = "type")]
    pub r#type: CodeDeltaType,
    /// The index of the output item in the response for which the code is being streamed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the code interpreter tool call item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The partial code snippet being streamed by the code interpreter.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCodeInterpreterCallCodeDeltaEvent {
    /// Emitted when a partial code snippet is streamed by the code interpreter.
    pub fn new(
        r#type: CodeDeltaType,
        output_index: i32,
        item_id: String,
        delta: String,
        sequence_number: i32,
    ) -> ResponseCodeInterpreterCallCodeDeltaEvent {
        ResponseCodeInterpreterCallCodeDeltaEvent {
            r#type,
            output_index,
            item_id,
            delta,
            sequence_number,
        }
    }
}

pub mod code_done_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.code_interpreter_call_code.done")]
        ResponseCodeInterpreterCallCodeDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCodeInterpreterCallCodeDone
        }
    }
}
pub use code_done_type::Type as CodeDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCodeInterpreterCallCodeDoneEvent {
    /// The type of the event. Always `response.code_interpreter_call_code.done`.
    #[serde(rename = "type")]
    pub r#type: CodeDoneType,
    /// The index of the output item in the response for which the code is finalized.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the code interpreter tool call item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The final code snippet output by the code interpreter.
    #[serde(rename = "code")]
    pub code: String,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCodeInterpreterCallCodeDoneEvent {
    /// Emitted when the code snippet is finalized by the code interpreter.
    pub fn new(
        r#type: CodeDoneType,
        output_index: i32,
        item_id: String,
        code: String,
        sequence_number: i32,
    ) -> ResponseCodeInterpreterCallCodeDoneEvent {
        ResponseCodeInterpreterCallCodeDoneEvent {
            r#type,
            output_index,
            item_id,
            code,
            sequence_number,
        }
    }
}

pub mod code_completed_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.code_interpreter_call.completed")]
        ResponseCodeInterpreterCallCompleted,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCodeInterpreterCallCompleted
        }
    }
}
pub use code_completed_type::Type as CodeCompletedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCodeInterpreterCallCompletedEvent {
    /// The type of the event. Always `response.code_interpreter_call.completed`.
    #[serde(rename = "type")]
    pub r#type: CodeCompletedType,
    /// The index of the output item in the response for which the code interpreter call is completed.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the code interpreter tool call item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCodeInterpreterCallCompletedEvent {
    /// Emitted when the code interpreter call is completed.
    pub fn new(
        r#type: CodeCompletedType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseCodeInterpreterCallCompletedEvent {
        ResponseCodeInterpreterCallCompletedEvent { r#type, output_index, item_id, sequence_number }
    }
}

pub mod code_in_progress_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.code_interpreter_call.in_progress")]
        ResponseCodeInterpreterCallInProgress,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCodeInterpreterCallInProgress
        }
    }
}
pub use code_in_progress_type::Type as CodeInProgressType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCodeInterpreterCallInProgressEvent {
    /// The type of the event. Always `response.code_interpreter_call.in_progress`.
    #[serde(rename = "type")]
    pub r#type: CodeInProgressType,
    /// The index of the output item in the response for which the code interpreter call is in progress.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the code interpreter tool call item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCodeInterpreterCallInProgressEvent {
    /// Emitted when a code interpreter call is in progress.
    pub fn new(
        r#type: CodeInProgressType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseCodeInterpreterCallInProgressEvent {
        ResponseCodeInterpreterCallInProgressEvent {
            r#type,
            output_index,
            item_id,
            sequence_number,
        }
    }
}

pub mod code_interpreting_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.code_interpreter_call.interpreting")]
        ResponseCodeInterpreterCallInterpreting,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseCodeInterpreterCallInterpreting
        }
    }
}
pub use code_interpreting_type::Type as CodeInterpretingType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCodeInterpreterCallInterpretingEvent {
    /// The type of the event. Always `response.code_interpreter_call.interpreting`.
    #[serde(rename = "type")]
    pub r#type: CodeInterpretingType,
    /// The index of the output item in the response for which the code interpreter is interpreting code.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the code interpreter tool call item.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event, used to order streaming events.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCodeInterpreterCallInterpretingEvent {
    /// Emitted when the code interpreter is actively interpreting the code snippet.
    pub fn new(
        r#type: CodeInterpretingType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseCodeInterpreterCallInterpretingEvent {
        ResponseCodeInterpreterCallInterpretingEvent {
            r#type,
            output_index,
            item_id,
            sequence_number,
        }
    }
}
