use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseImageGenCallCompletedEvent {
    /// The type of the event. Always 'response.image_generation_call.completed'.
    #[serde(rename = "type")]
    pub r#type: ImageGenCallCompletedType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The unique identifier of the image generation item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
}

impl ResponseImageGenCallCompletedEvent {
    /// Emitted when an image generation tool call has completed and the final image is available.
    pub fn new(
        r#type: ImageGenCallCompletedType,
        output_index: i32,
        sequence_number: i32,
        item_id: String,
    ) -> ResponseImageGenCallCompletedEvent {
        ResponseImageGenCallCompletedEvent { r#type, output_index, sequence_number, item_id }
    }
}

/// The type of the event. Always 'response.image_generation_call.completed'.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ImageGenCallCompletedType {
    #[serde(rename = "response.image_generation_call.completed")]
    #[default]
    ResponseImageGenerationCallCompleted,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseImageGenCallGeneratingEvent {
    /// The type of the event. Always 'response.image_generation_call.generating'.
    #[serde(rename = "type")]
    pub r#type: ImageGenCallGeneratingType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the image generation item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the image generation item being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseImageGenCallGeneratingEvent {
    /// Emitted when an image generation tool call is actively generating an image (intermediate state).
    pub fn new(
        r#type: ImageGenCallGeneratingType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseImageGenCallGeneratingEvent {
        ResponseImageGenCallGeneratingEvent { r#type, output_index, item_id, sequence_number }
    }
}

/// The type of the event. Always 'response.image_generation_call.generating'.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ImageGenCallGeneratingType {
    #[serde(rename = "response.image_generation_call.generating")]
    #[default]
    ResponseImageGenerationCallGenerating,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseImageGenCallInProgressEvent {
    /// The type of the event. Always 'response.image_generation_call.in_progress'.
    #[serde(rename = "type")]
    pub r#type: ImageGenCallInProgressType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the image generation item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the image generation item being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
}

impl ResponseImageGenCallInProgressEvent {
    /// Emitted when an image generation tool call is in progress.
    pub fn new(
        r#type: ImageGenCallInProgressType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
    ) -> ResponseImageGenCallInProgressEvent {
        ResponseImageGenCallInProgressEvent { r#type, output_index, item_id, sequence_number }
    }
}

/// The type of the event. Always 'response.image_generation_call.in_progress'.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ImageGenCallInProgressType {
    #[serde(rename = "response.image_generation_call.in_progress")]
    #[default]
    ResponseImageGenerationCallInProgress,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseImageGenCallPartialImageEvent {
    /// The type of the event. Always 'response.image_generation_call.partial_image'.
    #[serde(rename = "type")]
    pub r#type: ImageGenCallPartialImageType,
    /// The index of the output item in the response's output array.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// The unique identifier of the image generation item being processed.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The sequence number of the image generation item being processed.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// 0-based index for the partial image (backend is 1-based, but this is 0-based for the user).
    #[serde(rename = "partial_image_index")]
    pub partial_image_index: i32,
    /// Base64-encoded partial image data, suitable for rendering as an image.
    #[serde(rename = "partial_image_b64")]
    pub partial_image_b64: String,
}

impl ResponseImageGenCallPartialImageEvent {
    /// Emitted when a partial image is available during image generation streaming.
    pub fn new(
        r#type: ImageGenCallPartialImageType,
        output_index: i32,
        item_id: String,
        sequence_number: i32,
        partial_image_index: i32,
        partial_image_b64: String,
    ) -> ResponseImageGenCallPartialImageEvent {
        ResponseImageGenCallPartialImageEvent {
            r#type,
            output_index,
            item_id,
            sequence_number,
            partial_image_index,
            partial_image_b64,
        }
    }
}

/// The type of the event. Always 'response.image_generation_call.partial_image'.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ImageGenCallPartialImageType {
    #[serde(rename = "response.image_generation_call.partial_image")]
    #[default]
    ResponseImageGenerationCallPartialImage,
}
