use crate::models;
use serde::{Deserialize, Serialize};

pub mod web_search_completed_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.web_search_call.completed")]
        ResponseWebSearchCallCompleted,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseWebSearchCallCompleted
        }
    }
}
pub use web_search_completed_type::Type as WebSearchCompletedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseWebSearchCallCompletedEvent {
    /// The type of the event. Always `response.web_search_call.completed`.
    #[serde(rename = "type")]
    pub r#type: WebSearchCompletedType,
    /// The index of the output item that the web search call is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique ID for the output item associated with the web search call.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the web search call being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseWebSearchCallCompletedEvent {
    /// Emitted when a web search call is completed.
    pub fn new(
        r#type: WebSearchCompletedType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseWebSearchCallCompletedEvent {
        ResponseWebSearchCallCompletedEvent { r#type, output_index, item_id, sequence_number }
    }
}

pub mod web_search_in_progress_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.web_search_call.in_progress")]
        ResponseWebSearchCallInProgress,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseWebSearchCallInProgress
        }
    }
}
pub use web_search_in_progress_type::Type as WebSearchInProgressType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseWebSearchCallInProgressEvent {
    /// The type of the event. Always `response.web_search_call.in_progress`.
    #[serde(rename = "type")]
    pub r#type: WebSearchInProgressType,
    /// The index of the output item that the web search call is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique ID for the output item associated with the web search call.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the web search call being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseWebSearchCallInProgressEvent {
    /// Emitted when a web search call is initiated.
    pub fn new(
        r#type: WebSearchInProgressType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseWebSearchCallInProgressEvent {
        ResponseWebSearchCallInProgressEvent { r#type, output_index, item_id, sequence_number }
    }
}

pub mod web_search_searching_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.web_search_call.searching")]
        ResponseWebSearchCallSearching,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseWebSearchCallSearching
        }
    }
}
pub use web_search_searching_type::Type as WebSearchSearchingType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseWebSearchCallSearchingEvent {
    /// The type of the event. Always `response.web_search_call.searching`.
    #[serde(rename = "type")]
    pub r#type: WebSearchSearchingType,
    /// The index of the output item that the web search call is associated with.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique ID for the output item associated with the web search call.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the web search call being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseWebSearchCallSearchingEvent {
    /// Emitted when a web search call is executing.
    pub fn new(
        r#type: WebSearchSearchingType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseWebSearchCallSearchingEvent {
        ResponseWebSearchCallSearchingEvent { r#type, output_index, item_id, sequence_number }
    }
}
