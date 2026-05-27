use crate::models;
use serde::{Deserialize, Serialize};

use super::params::ImageParamsModeration;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditImageBodyJsonParam {
    /// Input image references to edit. For GPT image models, you can provide up to 16 images.
    #[serde(rename = "images")]
    pub images: Vec<models::ImageRefParam>,
    /// A text description of the desired image edit.
    #[serde(rename = "prompt")]
    pub prompt: String,
    #[serde(
        rename = "model",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub model: Option<Option<Box<models::EditImageBodyJsonParamModel>>>,
    #[serde(
        rename = "mask",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub mask: Option<Option<Box<models::ImageRefParam>>>,
    /// The number of edited images to generate.
    #[serde(
        rename = "n",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub n: Option<Option<i32>>,
    /// Output quality for GPT image models.
    #[serde(
        rename = "quality",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub quality: Option<Option<ImageEditQuality>>,
    /// Controls fidelity to the original input image(s).
    #[serde(
        rename = "input_fidelity",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_fidelity: Option<Option<EditImageInputFidelity>>,
    /// Requested output image size.
    #[serde(
        rename = "size",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub size: Option<Option<EditImageBodySize>>,
    /// A unique identifier representing your end-user, which can help OpenAI monitor and detect abuse.
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Output image format. Supported for GPT image models.
    #[serde(
        rename = "output_format",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_format: Option<Option<EditImageBodyOutputFormat>>,
    /// Compression level for `jpeg` or `webp` output.
    #[serde(
        rename = "output_compression",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_compression: Option<Option<i32>>,
    /// Moderation level for GPT image models.
    #[serde(
        rename = "moderation",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub moderation: Option<Option<ImageParamsModeration>>,
    /// Background behavior for generated image output.
    #[serde(
        rename = "background",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub background: Option<Option<EditImageBodyBackground>>,
    /// Stream partial image results as events.
    #[serde(
        rename = "stream",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream: Option<Option<bool>>,
    /// The number of partial images to generate. This parameter is used for streaming responses that return partial images. Value must be between 0 and 3. When set to 0, the response will be a single image sent in one streaming event.  Note that the final image may be sent before the full number of partial images are generated if the full image is generated more quickly.
    #[serde(
        rename = "partial_images",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub partial_images: Option<Option<i32>>,
}

impl EditImageBodyJsonParam {
    /// JSON request body for image edits.  Use `images` (array of `ImageRefParam`) instead of multipart `image` uploads. You can reference images via external URLs, data URLs, or uploaded file IDs. JSON edits support GPT image models only; DALL-E edits require multipart (`dall-e-2` only).
    pub fn new(images: Vec<models::ImageRefParam>, prompt: String) -> EditImageBodyJsonParam {
        EditImageBodyJsonParam {
            images,
            prompt,
            model: None,
            mask: None,
            n: None,
            quality: None,
            input_fidelity: None,
            size: None,
            user: None,
            output_format: None,
            output_compression: None,
            moderation: None,
            background: None,
            stream: None,
            partial_images: None,
        }
    }
}
/// Output quality for GPT image models.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditQuality {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditImageBodyJsonParamModel {}

impl EditImageBodyJsonParamModel {
    /// The model to use for image editing.
    pub fn new() -> EditImageBodyJsonParamModel {
        EditImageBodyJsonParamModel {}
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EditImageInputFidelity {
    #[serde(rename = "high")]
    #[default]
    High,
    #[serde(rename = "low")]
    Low,
}

/// Requested output image size.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EditImageBodySize {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "1024x1024")]
    Variant1024x1024,
    #[serde(rename = "1536x1024")]
    Variant1536x1024,
    #[serde(rename = "1024x1536")]
    Variant1024x1536,
}

/// Output image format for the edit body.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EditImageBodyOutputFormat {
    #[serde(rename = "png")]
    #[default]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

/// Background behavior for the edit body.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum EditImageBodyBackground {
    #[serde(rename = "transparent")]
    #[default]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageEditCompletedEvent {
    /// The type of the event. Always `image_edit.completed`.
    #[serde(rename = "type")]
    pub r#type: ImageEditCompletedEventType,
    /// Base64-encoded final edited image data, suitable for rendering as an image.
    #[serde(rename = "b64_json")]
    pub b64_json: String,
    /// The Unix timestamp when the event was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// The size of the edited image.
    #[serde(rename = "size")]
    pub size: ImageEditCompletedEventSize,
    /// The quality setting for the edited image.
    #[serde(rename = "quality")]
    pub quality: ImageEditCompletedEventQuality,
    /// The background setting for the edited image.
    #[serde(rename = "background")]
    pub background: ImageEditCompletedEventBackground,
    /// The output format for the edited image.
    #[serde(rename = "output_format")]
    pub output_format: ImageEditCompletedEventOutputFormat,
    #[serde(rename = "usage")]
    pub usage: Box<models::ImagesUsage>,
}

impl ImageEditCompletedEvent {
    /// Emitted when image editing has completed and the final image is available.
    pub fn new(
        r#type: ImageEditCompletedEventType,
        b64_json: String,
        created_at: i32,
        size: ImageEditCompletedEventSize,
        quality: ImageEditCompletedEventQuality,
        background: ImageEditCompletedEventBackground,
        output_format: ImageEditCompletedEventOutputFormat,
        usage: models::ImagesUsage,
    ) -> ImageEditCompletedEvent {
        ImageEditCompletedEvent {
            r#type,
            b64_json,
            created_at,
            size,
            quality,
            background,
            output_format,
            usage: Box::new(usage),
        }
    }
}
/// The type of the event. Always `image_edit.completed`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditCompletedEventType {
    #[serde(rename = "image_edit.completed")]
    #[default]
    ImageEditCompleted,
}

/// The size of the edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditCompletedEventSize {
    #[serde(rename = "1024x1024")]
    #[default]
    Variant1024x1024,
    #[serde(rename = "1024x1536")]
    Variant1024x1536,
    #[serde(rename = "1536x1024")]
    Variant1536x1024,
    #[serde(rename = "auto")]
    Auto,
}

/// The quality setting for the edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditCompletedEventQuality {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

/// The background setting for the edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditCompletedEventBackground {
    #[serde(rename = "transparent")]
    #[default]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

/// The output format for the edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditCompletedEventOutputFormat {
    #[serde(rename = "png")]
    #[default]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageEditPartialImageEvent {
    /// The type of the event. Always `image_edit.partial_image`.
    #[serde(rename = "type")]
    pub r#type: ImageEditPartialImageEventType,
    /// Base64-encoded partial image data, suitable for rendering as an image.
    #[serde(rename = "b64_json")]
    pub b64_json: String,
    /// The Unix timestamp when the event was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// The size of the requested edited image.
    #[serde(rename = "size")]
    pub size: ImageEditPartialImageEventSize,
    /// The quality setting for the requested edited image.
    #[serde(rename = "quality")]
    pub quality: ImageEditPartialImageEventQuality,
    /// The background setting for the requested edited image.
    #[serde(rename = "background")]
    pub background: ImageEditPartialImageEventBackground,
    /// The output format for the requested edited image.
    #[serde(rename = "output_format")]
    pub output_format: ImageEditPartialImageEventOutputFormat,
    /// 0-based index for the partial image (streaming).
    #[serde(rename = "partial_image_index")]
    pub partial_image_index: i32,
}

impl ImageEditPartialImageEvent {
    /// Emitted when a partial image is available during image editing streaming.
    pub fn new(
        r#type: ImageEditPartialImageEventType,
        b64_json: String,
        created_at: i32,
        size: ImageEditPartialImageEventSize,
        quality: ImageEditPartialImageEventQuality,
        background: ImageEditPartialImageEventBackground,
        output_format: ImageEditPartialImageEventOutputFormat,
        partial_image_index: i32,
    ) -> ImageEditPartialImageEvent {
        ImageEditPartialImageEvent {
            r#type,
            b64_json,
            created_at,
            size,
            quality,
            background,
            output_format,
            partial_image_index,
        }
    }
}
/// The type of the event. Always `image_edit.partial_image`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditPartialImageEventType {
    #[serde(rename = "image_edit.partial_image")]
    #[default]
    ImageEditPartialImage,
}

/// The size of the requested edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditPartialImageEventSize {
    #[serde(rename = "1024x1024")]
    #[default]
    Variant1024x1024,
    #[serde(rename = "1024x1536")]
    Variant1024x1536,
    #[serde(rename = "1536x1024")]
    Variant1536x1024,
    #[serde(rename = "auto")]
    Auto,
}

/// The quality setting for the requested edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditPartialImageEventQuality {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "auto")]
    Auto,
}

/// The background setting for the requested edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditPartialImageEventBackground {
    #[serde(rename = "transparent")]
    #[default]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

/// The output format for the requested edited image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageEditPartialImageEventOutputFormat {
    #[serde(rename = "png")]
    #[default]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageEditStreamEvent {}
