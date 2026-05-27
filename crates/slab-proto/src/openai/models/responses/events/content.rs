use crate::models;
use serde::{Deserialize, Serialize};

pub mod content_part_added_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.content_part.added")]
        ResponseContentPartAdded,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseContentPartAdded
        }
    }
}
pub use content_part_added_type::Type as ContentPartAddedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseContentPartAddedEvent {
    /// The type of the event. Always `response.content_part.added`.
    #[serde(rename = "type")]
    pub r#type: ContentPartAddedType,
    /// The ID of the output item that the content part was added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the content part was added to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that was added.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The content part that was added.
    #[serde(rename = "part")]
    pub part: Box<models::OutputContent>,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseContentPartAddedEvent {
    /// Emitted when a new content part is added.
    pub fn new(
        r#type: ContentPartAddedType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        part: models::OutputContent,
        sequence_number: i32,
    ) -> ResponseContentPartAddedEvent {
        ResponseContentPartAddedEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            part: Box::new(part),
            sequence_number,
        }
    }
}

pub mod content_part_done_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.content_part.done")]
        ResponseContentPartDone,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseContentPartDone
        }
    }
}
pub use content_part_done_type::Type as ContentPartDoneType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseContentPartDoneEvent {
    /// The type of the event. Always `response.content_part.done`.
    #[serde(rename = "type")]
    pub r#type: ContentPartDoneType,
    /// The ID of the output item that the content part was added to.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item that the content part was added to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part that is done.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The content part that is done.
    #[serde(rename = "part")]
    pub part: Box<models::OutputContent>,
}

impl ResponseContentPartDoneEvent {
    /// Emitted when a content part is done.
    pub fn new(
        r#type: ContentPartDoneType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        sequence_number: i32,
        part: models::OutputContent,
    ) -> ResponseContentPartDoneEvent {
        ResponseContentPartDoneEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            sequence_number,
            part: Box::new(part),
        }
    }
}

pub mod annotation_added_type {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Type {
        #[serde(rename = "response.output_text.annotation.added")]
        ResponseOutputTextAnnotationAdded,
    }
    impl Default for Type {
        fn default() -> Self {
            Self::ResponseOutputTextAnnotationAdded
        }
    }
}
pub use annotation_added_type::Type as AnnotationAddedType;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseOutputTextAnnotationAddedEvent {
    /// The type of the event. Always 'response.output_text.annotation.added'.
    #[serde(rename = "type")]
    pub r#type: AnnotationAddedType,
    /// The unique identifier of the item to which the annotation is being added.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The index of the content part within the output item.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    /// The index of the annotation within the content part.
    #[serde(rename = "annotation_index")]
    pub annotation_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The annotation object being added. (See annotation schema for details.)
    #[serde(rename = "annotation")]
    pub annotation: serde_json::Value,
}

impl ResponseOutputTextAnnotationAddedEvent {
    /// Emitted when an annotation is added to output text content.
    pub fn new(
        r#type: AnnotationAddedType,
        item_id: String,
        output_index: i32,
        content_index: i32,
        annotation_index: i32,
        sequence_number: i32,
        annotation: serde_json::Value,
    ) -> ResponseOutputTextAnnotationAddedEvent {
        ResponseOutputTextAnnotationAddedEvent {
            r#type,
            item_id,
            output_index,
            content_index,
            annotation_index,
            sequence_number,
            annotation,
        }
    }
}
