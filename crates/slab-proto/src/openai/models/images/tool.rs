use crate::openai::models;
use serde::{Deserialize, Serialize};

use super::params::ImageParamsModeration;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageGenTool {
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::ImageGenToolModel>>,
    /// The quality of the generated image. One of `low`, `medium`, `high`, or `auto`. Default: `auto`.
    #[serde(rename = "quality", skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageGenToolQuality>,
    #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
    pub size: Option<Box<models::ImageGenToolSize>>,
    /// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.
    #[serde(rename = "output_format", skip_serializing_if = "Option::is_none")]
    pub output_format: Option<ImageGenToolOutputFormat>,
    /// Compression level for the output image. Default: 100.
    #[serde(rename = "output_compression", skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<i32>,
    /// Moderation level for the generated image. Default: `auto`.
    #[serde(rename = "moderation", skip_serializing_if = "Option::is_none")]
    pub moderation: Option<ImageParamsModeration>,
    /// Background type for the generated image. One of `transparent`, `opaque`, or `auto`. Default: `auto`.
    #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
    pub background: Option<ImageGenToolBackground>,
    #[serde(
        rename = "input_fidelity",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_fidelity: Option<Option<models::InputFidelity>>,
    #[serde(rename = "input_image_mask", skip_serializing_if = "Option::is_none")]
    pub input_image_mask: Option<Box<models::ImageGenToolInputImageMask>>,
    /// Number of partial images to generate in streaming mode, from 0 (default value) to 3.
    #[serde(rename = "partial_images", skip_serializing_if = "Option::is_none")]
    pub partial_images: Option<i32>,
    /// Whether to generate a new image or edit an existing image. Default: `auto`.
    #[serde(rename = "action", skip_serializing_if = "Option::is_none")]
    pub action: Option<models::ImageGenActionEnum>,
    /// Number of images to generate.
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
}

impl ImageGenTool {
    /// A tool that generates images using the GPT image models.
    pub fn new() -> ImageGenTool {
        ImageGenTool {
            model: None,
            quality: None,
            size: None,
            output_format: None,
            output_compression: None,
            moderation: None,
            background: None,
            input_fidelity: None,
            input_image_mask: None,
            partial_images: None,
            action: None,
            n: None,
        }
    }
}

/// The quality of the generated image. One of `low`, `medium`, `high`, or `auto`. Default: `auto`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageGenToolQuality {
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

/// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageGenToolOutputFormat {
    #[serde(rename = "png")]
    #[default]
    Png,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
}

/// Background type for the generated image.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageGenToolBackground {
    #[serde(rename = "transparent")]
    #[default]
    Transparent,
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "auto")]
    Auto,
}

use super::params::ImageParamsStatus;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageGenToolCall {
    /// The type of the image generation call. Always `image_generation_call`.
    #[serde(rename = "type")]
    pub r#type: ImageGenToolCallType,
    /// The unique ID of the image generation call.
    #[serde(rename = "id")]
    pub id: String,
    /// The status of the image generation call.
    #[serde(rename = "status")]
    pub status: ImageParamsStatus,
    /// The generated image encoded in base64.
    #[serde(rename = "result", deserialize_with = "Option::deserialize")]
    pub result: Option<String>,
    /// Background transparency (`opaque` / `transparent` / `auto`).
    #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Output format (`webp` / `png` / `jpeg`).
    #[serde(rename = "output_format", skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    /// Quality level (`low` / `medium` / `high` / `auto`).
    #[serde(rename = "quality", skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// The revised prompt used for image generation.
    #[serde(rename = "revised_prompt", skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
    /// Image dimensions (e.g. `1024x1024`).
    #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

impl ImageGenToolCall {
    /// An image generation request made by the model.
    pub fn new(
        r#type: ImageGenToolCallType,
        id: String,
        status: ImageParamsStatus,
        result: Option<String>,
    ) -> ImageGenToolCall {
        ImageGenToolCall {
            r#type,
            id,
            status,
            result,
            background: None,
            output_format: None,
            quality: None,
            revised_prompt: None,
            size: None,
        }
    }
}
/// The type of the image generation call. Always `image_generation_call`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageGenToolCallType {
    #[serde(rename = "image_generation_call")]
    #[default]
    ImageGenerationCall,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageGenToolInputImageMask {
    /// Base64-encoded mask image.
    #[serde(rename = "image_url", skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// File ID for the mask image.
    #[serde(rename = "file_id", skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

impl ImageGenToolInputImageMask {
    /// Optional mask for inpainting. Contains `image_url` (string, optional) and `file_id` (string, optional).
    pub fn new() -> ImageGenToolInputImageMask {
        ImageGenToolInputImageMask { image_url: None, file_id: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageGenToolModel {}

impl ImageGenToolModel {
    pub fn new() -> ImageGenToolModel {
        ImageGenToolModel {}
    }
}

/// Image size as a bare string (e.g. `"1024x1024"`). Transparent newtype so
/// it serializes as the inner string, matching the OpenAI wire format.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageGenToolSize(pub String);

impl ImageGenToolSize {
    pub fn new() -> ImageGenToolSize {
        ImageGenToolSize(String::new())
    }
    pub fn from_string(s: String) -> ImageGenToolSize {
        ImageGenToolSize(s)
    }
}
