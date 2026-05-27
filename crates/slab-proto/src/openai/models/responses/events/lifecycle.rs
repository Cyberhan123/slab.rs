use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCompletedEvent {
    /// The type of the event. Always `response.completed`.
    #[serde(rename = "type")]
    pub r#type: ResponseCompletedType,
    /// Properties of the completed response.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The sequence number for this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCompletedEvent {
    /// Emitted when the model response is complete.
    pub fn new(
        r#type: ResponseCompletedType,
        response: models::Response,
        sequence_number: i32,
    ) -> ResponseCompletedEvent {
        ResponseCompletedEvent { r#type, response: Box::new(response), sequence_number }
    }
}
pub mod response_completed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.completed`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.completed")]
        #[default]
        ResponseCompleted,
    }
    
}
pub use response_completed_type::Type as ResponseCompletedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseCreatedEvent {
    /// The type of the event. Always `response.created`.
    #[serde(rename = "type")]
    pub r#type: ResponseCreatedType,
    /// The response that was created.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The sequence number for this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseCreatedEvent {
    /// An event that is emitted when a response is created.
    pub fn new(
        r#type: ResponseCreatedType,
        response: models::Response,
        sequence_number: i32,
    ) -> ResponseCreatedEvent {
        ResponseCreatedEvent { r#type, response: Box::new(response), sequence_number }
    }
}
pub mod response_created_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.created`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.created")]
        #[default]
        ResponseCreated,
    }
    
}
pub use response_created_type::Type as ResponseCreatedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseErrorEvent {
    /// The type of the event. Always `error`.
    #[serde(rename = "type")]
    pub r#type: ErrorType,
    /// The error code.
    #[serde(rename = "code", deserialize_with = "Option::deserialize")]
    pub code: Option<String>,
    /// The error message.
    #[serde(rename = "message")]
    pub message: String,
    /// The error parameter.
    #[serde(rename = "param", deserialize_with = "Option::deserialize")]
    pub param: Option<String>,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseErrorEvent {
    /// Emitted when an error occurs.
    pub fn new(
        r#type: ErrorType,
        code: Option<String>,
        message: String,
        param: Option<String>,
        sequence_number: i32,
    ) -> ResponseErrorEvent {
        ResponseErrorEvent { r#type, code, message, param, sequence_number }
    }
}
pub mod error_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `error`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "error")]
        #[default]
        Error,
    }
    
}
pub use error_type::Type as ErrorType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseFailedEvent {
    /// The type of the event. Always `response.failed`.
    #[serde(rename = "type")]
    pub r#type: ResponseFailedType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The response that failed.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
}

impl ResponseFailedEvent {
    /// An event that is emitted when a response fails.
    pub fn new(
        r#type: ResponseFailedType,
        sequence_number: i32,
        response: models::Response,
    ) -> ResponseFailedEvent {
        ResponseFailedEvent { r#type, sequence_number, response: Box::new(response) }
    }
}
pub mod response_failed_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.failed`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.failed")]
        #[default]
        ResponseFailed,
    }
    
}
pub use response_failed_type::Type as ResponseFailedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseInProgressEvent {
    /// The type of the event. Always `response.in_progress`.
    #[serde(rename = "type")]
    pub r#type: ResponseInProgressType,
    /// The response that is in progress.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseInProgressEvent {
    /// Emitted when the response is in progress.
    pub fn new(
        r#type: ResponseInProgressType,
        response: models::Response,
        sequence_number: i32,
    ) -> ResponseInProgressEvent {
        ResponseInProgressEvent { r#type, response: Box::new(response), sequence_number }
    }
}
pub mod response_in_progress_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.in_progress`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.in_progress")]
        #[default]
        ResponseInProgress,
    }
    
}
pub use response_in_progress_type::Type as ResponseInProgressType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseIncompleteEvent {
    /// The type of the event. Always `response.incomplete`.
    #[serde(rename = "type")]
    pub r#type: ResponseIncompleteType,
    /// The response that was incomplete.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseIncompleteEvent {
    /// An event that is emitted when a response finishes as incomplete.
    pub fn new(
        r#type: ResponseIncompleteType,
        response: models::Response,
        sequence_number: i32,
    ) -> ResponseIncompleteEvent {
        ResponseIncompleteEvent { r#type, response: Box::new(response), sequence_number }
    }
}
pub mod response_incomplete_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always `response.incomplete`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.incomplete")]
        #[default]
        ResponseIncomplete,
    }
    
}
pub use response_incomplete_type::Type as ResponseIncompleteType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseQueuedEvent {
    /// The type of the event. Always 'response.queued'.
    #[serde(rename = "type")]
    pub r#type: ResponseQueuedType,
    /// The full response object that is queued.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The sequence number for this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseQueuedEvent {
    /// Emitted when a response is queued and waiting to be processed.
    pub fn new(
        r#type: ResponseQueuedType,
        response: models::Response,
        sequence_number: i32,
    ) -> ResponseQueuedEvent {
        ResponseQueuedEvent { r#type, response: Box::new(response), sequence_number }
    }
}
pub mod response_queued_type {
    use serde::{Deserialize, Serialize};
    /// The type of the event. Always 'response.queued'.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Type {
        #[serde(rename = "response.queued")]
        #[default]
        ResponseQueued,
    }
    
}
pub use response_queued_type::Type as ResponseQueuedType;
