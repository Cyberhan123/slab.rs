use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryPartAddedEvent {
    /// The type of the event. Always `response.reasoning_summary_part.added`.
    #[serde(rename = "type")]
    pub r#type: ReasoningSummaryPartAddedType,
    /// The ID of the item this summary part is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this summary part is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the summary part within the reasoning summary.
    #[serde(rename = "summary_index")]
    pub summary_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    #[serde(rename = "part")]
    pub part: Box<models::ResponseReasoningSummaryPartAddedEventPart>,
}

impl ResponseReasoningSummaryPartAddedEvent {
    /// Emitted when a new reasoning summary part is added.
    pub fn new(
        r#type: ReasoningSummaryPartAddedType,
        item_id: String,
        output_index: i32,
        summary_index: i32,
        sequence_number: i32,
        part: models::ResponseReasoningSummaryPartAddedEventPart,
    ) -> ResponseReasoningSummaryPartAddedEvent {
        ResponseReasoningSummaryPartAddedEvent {
            r#type,
            item_id,
            output_index,
            summary_index,
            sequence_number,
            part: Box::new(part),
        }
    }
}
pub mod reasoning_summary_part_added_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_summary_part.added`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_summary_part.added")]
        ResponseReasoningSummaryPartAdded,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningSummaryPartAdded
        }
    }
}
pub use reasoning_summary_part_added_type::Type as ReasoningSummaryPartAddedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryPartAddedEventPart {
    /// The type of the summary part. Always `summary_text`.
    #[serde(rename = "type")]
    pub r#type: SummaryTextType,
    /// The text of the summary part.
    #[serde(rename = "text")]
    pub text: String,
}

impl ResponseReasoningSummaryPartAddedEventPart {
    /// The summary part that was added.
    pub fn new(
        r#type: SummaryTextType,
        text: String,
    ) -> ResponseReasoningSummaryPartAddedEventPart {
        ResponseReasoningSummaryPartAddedEventPart { r#type, text }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryPartDoneEvent {
    /// The type of the event. Always `response.reasoning_summary_part.done`.
    #[serde(rename = "type")]
    pub r#type: ReasoningSummaryPartDoneType,
    /// The ID of the item this summary part is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this summary part is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the summary part within the reasoning summary.
    #[serde(rename = "summary_index")]
    pub summary_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    #[serde(rename = "part")]
    pub part: Box<models::ResponseReasoningSummaryPartDoneEventPart>,
}

impl ResponseReasoningSummaryPartDoneEvent {
    /// Emitted when a reasoning summary part is completed.
    pub fn new(
        r#type: ReasoningSummaryPartDoneType,
        item_id: String,
        output_index: i32,
        summary_index: i32,
        sequence_number: i32,
        part: models::ResponseReasoningSummaryPartDoneEventPart,
    ) -> ResponseReasoningSummaryPartDoneEvent {
        ResponseReasoningSummaryPartDoneEvent {
            r#type,
            item_id,
            output_index,
            summary_index,
            sequence_number,
            part: Box::new(part),
        }
    }
}
pub mod reasoning_summary_part_done_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_summary_part.done`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_summary_part.done")]
        ResponseReasoningSummaryPartDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningSummaryPartDone
        }
    }
}
pub use reasoning_summary_part_done_type::Type as ReasoningSummaryPartDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryPartDoneEventPart {
    /// The type of the summary part. Always `summary_text`.
    #[serde(rename = "type")]
    pub r#type: SummaryTextType,
    /// The text of the summary part.
    #[serde(rename = "text")]
    pub text: String,
}

impl ResponseReasoningSummaryPartDoneEventPart {
    /// The completed summary part.
    pub fn new(r#type: SummaryTextType, text: String) -> ResponseReasoningSummaryPartDoneEventPart {
        ResponseReasoningSummaryPartDoneEventPart { r#type, text }
    }
}
pub mod summary_text_type {
    use serde::{Deserialize, Serialize};
    /// The type of the summary part. Always `summary_text`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "summary_text")]
        SummaryText,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::SummaryText
        }
    }
}
pub use summary_text_type::Type as SummaryTextType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryTextDeltaEvent {
    /// The type of the event. Always `response.reasoning_summary_text.delta`.
    #[serde(rename = "type")]
    pub r#type: ReasoningSummaryTextDeltaType,
    /// The ID of the item this summary text delta is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this summary text delta is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the summary part within the reasoning summary.
    #[serde(rename = "summary_index")]
    pub summary_index: i32,
    /// The text delta that was added to the summary.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseReasoningSummaryTextDeltaEvent {
    /// Emitted when a delta is added to a reasoning summary text.
    pub fn new(
        r#type: ReasoningSummaryTextDeltaType,
        item_id: String,
        output_index: i32,
        summary_index: i32,
        delta: String,
        sequence_number: i32,
    ) -> ResponseReasoningSummaryTextDeltaEvent {
        ResponseReasoningSummaryTextDeltaEvent {
            r#type,
            item_id,
            output_index,
            summary_index,
            delta,
            sequence_number,
        }
    }
}
pub mod reasoning_summary_text_delta_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_summary_text.delta`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_summary_text.delta")]
        ResponseReasoningSummaryTextDelta,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningSummaryTextDelta
        }
    }
}
pub use reasoning_summary_text_delta_type::Type as ReasoningSummaryTextDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningSummaryTextDoneEvent {
    /// The type of the event. Always `response.reasoning_summary_text.done`.
    #[serde(rename = "type")]
    pub r#type: ReasoningSummaryTextDoneType,
    /// The ID of the item this summary text is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this summary text is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the summary part within the reasoning summary.
    #[serde(rename = "summary_index")]
    pub summary_index: i32,
    /// The full text of the completed reasoning summary.
    #[serde(rename = "text")]
    pub text: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseReasoningSummaryTextDoneEvent {
    /// Emitted when a reasoning summary text is completed.
    pub fn new(
        r#type: ReasoningSummaryTextDoneType,
        item_id: String,
        output_index: i32,
        summary_index: i32,
        text: String,
        sequence_number: i32,
    ) -> ResponseReasoningSummaryTextDoneEvent {
        ResponseReasoningSummaryTextDoneEvent {
            r#type,
            item_id,
            output_index,
            summary_index,
            text,
            sequence_number,
        }
    }
}
pub mod reasoning_summary_text_done_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_summary_text.done`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_summary_text.done")]
        ResponseReasoningSummaryTextDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningSummaryTextDone
        }
    }
}
pub use reasoning_summary_text_done_type::Type as ReasoningSummaryTextDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningTextDeltaEvent {
    /// The type of the event. Always `response.reasoning_text.delta`.
    #[serde(rename = "type")]
    pub r#type: ReasoningTextDeltaType,
    /// The ID of the item this reasoning text delta is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this reasoning text delta is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the reasoning content part this delta is associated with.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The text delta that was added to the reasoning content.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseReasoningTextDeltaEvent {
    /// Emitted when a delta is added to a reasoning text.
    pub fn new(
        r#type: ReasoningTextDeltaType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
        sequence_number: i32,
    ) -> ResponseReasoningTextDeltaEvent {
        ResponseReasoningTextDeltaEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            delta,
            sequence_number,
        }
    }
}
pub mod reasoning_text_delta_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_text.delta`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_text.delta")]
        ResponseReasoningTextDelta,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningTextDelta
        }
    }
}
pub use reasoning_text_delta_type::Type as ReasoningTextDeltaType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseReasoningTextDoneEvent {
    /// The type of the event. Always `response.reasoning_text.done`.
    #[serde(rename = "type")]
    pub r#type: ReasoningTextDoneType,
    /// The ID of the item this reasoning text is associated with.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item this reasoning text is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the reasoning content part.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The full text of the completed reasoning content.
    #[serde(rename = "text")]
    pub text: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseReasoningTextDoneEvent {
    /// Emitted when a reasoning text is completed.
    pub fn new(
        r#type: ReasoningTextDoneType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        text: String,
        sequence_number: i32,
    ) -> ResponseReasoningTextDoneEvent {
        ResponseReasoningTextDoneEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            text,
            sequence_number,
        }
    }
}
pub mod reasoning_text_done_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.reasoning_text.done`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.reasoning_text.done")]
        ResponseReasoningTextDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseReasoningTextDone
        }
    }
}
pub use reasoning_text_done_type::Type as ReasoningTextDoneType;
