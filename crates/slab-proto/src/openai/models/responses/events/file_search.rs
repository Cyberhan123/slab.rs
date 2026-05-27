use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFileSearchCallCompletedEvent {
    /// The type of the event. Always `response.file_search_call.completed`.
    #[serde(rename = "type")]
    pub r#type: ResponseFileSearchCallCompletedEventType,
    /// The index of the output item that the file search call is initiated.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The ID of the output item that the file search call is initiated.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseFileSearchCallCompletedEvent {
    /// Emitted when a file search call is completed (results found).
    pub fn new(
        r#type: ResponseFileSearchCallCompletedEventType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseFileSearchCallCompletedEvent {
        ResponseFileSearchCallCompletedEvent { r#type, output_index, item_id, sequence_number }
    }
}
/// The type of the event. Always `response.file_search_call.completed`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseFileSearchCallCompletedEventType {
    #[serde(rename = "response.file_search_call.completed")]
    #[default]
    ResponseFileSearchCallCompleted,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFileSearchCallInProgressEvent {
    /// The type of the event. Always `response.file_search_call.in_progress`.
    #[serde(rename = "type")]
    pub r#type: ResponseFileSearchCallInProgressEventType,
    /// The index of the output item that the file search call is initiated.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The ID of the output item that the file search call is initiated.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseFileSearchCallInProgressEvent {
    /// Emitted when a file search call is initiated.
    pub fn new(
        r#type: ResponseFileSearchCallInProgressEventType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseFileSearchCallInProgressEvent {
        ResponseFileSearchCallInProgressEvent { r#type, output_index, item_id, sequence_number }
    }
}
/// The type of the event. Always `response.file_search_call.in_progress`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseFileSearchCallInProgressEventType {
    #[serde(rename = "response.file_search_call.in_progress")]
    #[default]
    ResponseFileSearchCallInProgress,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFileSearchCallSearchingEvent {
    /// The type of the event. Always `response.file_search_call.searching`.
    #[serde(rename = "type")]
    pub r#type: ResponseFileSearchCallSearchingEventType,
    /// The index of the output item that the file search call is searching.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The ID of the output item that the file search call is initiated.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseFileSearchCallSearchingEvent {
    /// Emitted when a file search is currently searching.
    pub fn new(
        r#type: ResponseFileSearchCallSearchingEventType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseFileSearchCallSearchingEvent {
        ResponseFileSearchCallSearchingEvent { r#type, output_index, item_id, sequence_number }
    }
}
/// The type of the event. Always `response.file_search_call.searching`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseFileSearchCallSearchingEventType {
    #[serde(rename = "response.file_search_call.searching")]
    #[default]
    ResponseFileSearchCallSearching,
}

