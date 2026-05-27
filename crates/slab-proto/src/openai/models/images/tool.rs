pub mod image_gen_tool {
    use crate::models;
    use serde::{Deserialize, Serialize};

    use super::super::params::Moderation;
    use super::super::params::Status;
    use super::super::resource::Background;
    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenTool {
        /// The type of the image generation tool. Always `image_generation`.
        #[serde(rename = "type")]
        pub r#type: Type,
        #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
        pub model: Option<Box<models::ImageGenToolModel>>,
        /// The quality of the generated image. One of `low`, `medium`, `high`, or `auto`. Default: `auto`.
        #[serde(rename = "quality", skip_serializing_if = "Option::is_none")]
        pub quality: Option<Quality>,
        #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
        pub size: Option<Box<models::ImageGenToolSize>>,
        /// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.
        #[serde(rename = "output_format", skip_serializing_if = "Option::is_none")]
        pub output_format: Option<OutputFormat>,
        /// Compression level for the output image. Default: 100.
        #[serde(rename = "output_compression", skip_serializing_if = "Option::is_none")]
        pub output_compression: Option<i32>,
        /// Moderation level for the generated image. Default: `auto`.
        #[serde(rename = "moderation", skip_serializing_if = "Option::is_none")]
        pub moderation: Option<Moderation>,
        /// Background type for the generated image. One of `transparent`, `opaque`, or `auto`. Default: `auto`.
        #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
        pub background: Option<Background>,
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
    }

    impl ImageGenTool {
        /// A tool that generates images using the GPT image models.
        pub fn new(r#type: Type) -> ImageGenTool {
            ImageGenTool {
                r#type,
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
            }
        }
    }
    /// The type of the image generation tool. Always `image_generation`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Type {
        #[serde(rename = "image_generation")]
        ImageGeneration,
    }

    impl Default for Type {
        fn default() -> Type {
            Self::ImageGeneration
        }
    }
    /// The quality of the generated image. One of `low`, `medium`, `high`, or `auto`. Default: `auto`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Quality {
        #[serde(rename = "low")]
        Low,
        #[serde(rename = "medium")]
        Medium,
        #[serde(rename = "high")]
        High,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Quality {
        fn default() -> Quality {
            Self::Low
        }
    }
    /// The output format of the generated image. One of `png`, `webp`, or `jpeg`. Default: `png`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum OutputFormat {
        #[serde(rename = "png")]
        Png,
        #[serde(rename = "webp")]
        Webp,
        #[serde(rename = "jpeg")]
        Jpeg,
    }

    impl Default for OutputFormat {
        fn default() -> OutputFormat {
            Self::Png
        }
    }
    // Moderation level for the generated image. Default: `auto`.
}

pub mod image_gen_tool_call {
    use crate::models;
    use serde::{Deserialize, Serialize};

    use super::super::params::Status;
    use super::super::resource::Background;
    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenToolCall {
        /// The type of the image generation call. Always `image_generation_call`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// The unique ID of the image generation call.
        #[serde(rename = "id")]
        pub id: String,
        /// The status of the image generation call.
        #[serde(rename = "status")]
        pub status: Status,
        /// The generated image encoded in base64.
        #[serde(rename = "result", deserialize_with = "Option::deserialize")]
        pub result: Option<String>,
    }

    impl ImageGenToolCall {
        /// An image generation request made by the model.
        pub fn new(
            r#type: Type,
            id: String,
            status: Status,
            result: Option<String>,
        ) -> ImageGenToolCall {
            ImageGenToolCall { r#type, id, status, result }
        }
    }
    /// The type of the image generation call. Always `image_generation_call`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Type {
        #[serde(rename = "image_generation_call")]
        ImageGenerationCall,
    }

    impl Default for Type {
        fn default() -> Type {
            Self::ImageGenerationCall
        }
    }
    // The status of the image generation call.
}

pub mod image_gen_tool_input_image_mask {
    use crate::models;
    use serde::{Deserialize, Serialize};

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
}

pub mod image_gen_tool_model {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenToolModel {}

    impl ImageGenToolModel {
        pub fn new() -> ImageGenToolModel {
            ImageGenToolModel {}
        }
    }
}

pub mod image_gen_tool_size {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenToolSize {}

    impl ImageGenToolSize {
        /// The size of the generated images. For `gpt-image-2` and `gpt-image-2-2026-04-21`, arbitrary resolutions are supported as `WIDTHxHEIGHT` strings, for example `1536x864`. Width and height must both be divisible by 16 and the requested aspect ratio must be between 1:3 and 3:1. Resolutions above `2560x1440` are experimental, and the maximum supported resolution is `3840x2160`. The requested size must also satisfy the model's current pixel and edge limits. The standard sizes `1024x1024`, `1536x1024`, and `1024x1536` are supported by the GPT image models; `auto` is supported for models that allow automatic sizing. For `dall-e-2`, use one of `256x256`, `512x512`, or `1024x1024`. For `dall-e-3`, use one of `1024x1024`, `1792x1024`, or `1024x1792`.
        pub fn new() -> ImageGenToolSize {
            ImageGenToolSize {}
        }
    }
}

pub use image_gen_tool::ImageGenTool;
pub(crate) use image_gen_tool::OutputFormat;
pub(crate) use image_gen_tool::Quality;
pub(crate) use image_gen_tool::Type;
pub use image_gen_tool_call::ImageGenToolCall;
pub use image_gen_tool_input_image_mask::ImageGenToolInputImageMask;
pub use image_gen_tool_model::ImageGenToolModel;
pub use image_gen_tool_size::ImageGenToolSize;
