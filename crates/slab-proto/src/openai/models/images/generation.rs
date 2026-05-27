pub mod create {
    /*
     * OpenAI API - Merged type definitions
     */

    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageEditRequestImage {}

    impl CreateImageEditRequestImage {
        /// The image(s) to edit. Must be a supported image file or an array of images.  For the GPT image models (`gpt-image-1`, `gpt-image-1-mini`, and `gpt-image-1.5`), each image should be a `png`, `webp`, or `jpg` file less than 50MB. You can provide up to 16 images. `chatgpt-image-latest` follows the same input constraints as GPT image models.  For `dall-e-2`, you can only provide one image, and it should be a square `png` file less than 4MB.
        pub fn new() -> CreateImageEditRequestImage {
            CreateImageEditRequestImage {}
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageEditRequestModel {}

    impl CreateImageEditRequestModel {
        /// The model to use for image generation. Defaults to `gpt-image-1.5`.
        pub fn new() -> CreateImageEditRequestModel {
            CreateImageEditRequestModel {}
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageEditRequestSize {}

    impl CreateImageEditRequestSize {
        /// The size of the generated images. For `gpt-image-2` and `gpt-image-2-2026-04-21`, arbitrary resolutions are supported as `WIDTHxHEIGHT` strings, for example `1536x864`. Width and height must both be divisible by 16 and the requested aspect ratio must be between 1:3 and 3:1. Resolutions above `2560x1440` are experimental, and the maximum supported resolution is `3840x2160`. The requested size must also satisfy the model's current pixel and edge limits. The standard sizes `1024x1024`, `1536x1024`, and `1024x1536` are supported by the GPT image models; `auto` is supported for models that allow automatic sizing. For `dall-e-2`, use one of `256x256`, `512x512`, or `1024x1024`. For `dall-e-3`, use one of `1024x1024`, `1792x1024`, or `1024x1792`.
        pub fn new() -> CreateImageEditRequestSize {
            CreateImageEditRequestSize {}
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageRequest {
        /// A text description of the desired image(s). The maximum length is 32000 characters for the GPT image models, 1000 characters for `dall-e-2` and 4000 characters for `dall-e-3`.
        #[serde(rename = "prompt")]
        pub prompt: String,
        #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
        pub model: Option<Box<models::CreateImageRequestModel>>,
        /// The number of images to generate. Must be between 1 and 10. For `dall-e-3`, only `n=1` is supported.
        #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
        pub n: Option<i32>,
        /// The quality of the image that will be generated.  - `auto` (default value) will automatically select the best quality for the given model. - `high`, `medium` and `low` are supported for the GPT image models. - `hd` and `standard` are supported for `dall-e-3`. - `standard` is the only option for `dall-e-2`.
        #[serde(rename = "quality", skip_serializing_if = "Option::is_none")]
        pub quality: Option<Quality>,
        /// The format in which generated images with `dall-e-2` and `dall-e-3` are returned. Must be one of `url` or `b64_json`. URLs are only valid for 60 minutes after the image has been generated. This parameter isn't supported for the GPT image models, which always return base64-encoded images.
        #[serde(rename = "response_format", skip_serializing_if = "Option::is_none")]
        pub response_format: Option<ResponseFormat>,
        /// The format in which the generated images are returned. This parameter is only supported for the GPT image models. Must be one of `png`, `jpeg`, or `webp`.
        #[serde(rename = "output_format", skip_serializing_if = "Option::is_none")]
        pub output_format: Option<OutputFormat>,
        /// The compression level (0-100%) for the generated images. This parameter is only supported for the GPT image models with the `webp` or `jpeg` output formats, and defaults to 100.
        #[serde(rename = "output_compression", skip_serializing_if = "Option::is_none")]
        pub output_compression: Option<i32>,
        /// Generate the image in streaming mode. Defaults to `false`. See the [Image generation guide](/docs/guides/image-generation) for more information. This parameter is only supported for the GPT image models.
        #[serde(rename = "stream", skip_serializing_if = "Option::is_none")]
        pub stream: Option<bool>,
        /// The number of partial images to generate. This parameter is used for streaming responses that return partial images. Value must be between 0 and 3. When set to 0, the response will be a single image sent in one streaming event.  Note that the final image may be sent before the full number of partial images are generated if the full image is generated more quickly.
        #[serde(
            rename = "partial_images",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub partial_images: Option<Option<i32>>,
        #[serde(rename = "size", skip_serializing_if = "Option::is_none")]
        pub size: Option<Box<models::CreateImageRequestSize>>,
        /// Control the content-moderation level for images generated by the GPT image models. Must be either `low` for less restrictive filtering or `auto` (default value).
        #[serde(rename = "moderation", skip_serializing_if = "Option::is_none")]
        pub moderation: Option<Moderation>,
        /// Allows to set transparency for the background of the generated image(s). This parameter is only supported for the GPT image models. Must be one of `transparent`, `opaque` or `auto` (default value). When `auto` is used, the model will automatically determine the best background for the image.  If `transparent`, the output format needs to support transparency, so it should be set to either `png` (default value) or `webp`.
        #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
        pub background: Option<Background>,
        /// The style of the generated images. This parameter is only supported for `dall-e-3`. Must be one of `vivid` or `natural`. Vivid causes the model to lean towards generating hyper-real and dramatic images. Natural causes the model to produce more natural, less hyper-real looking images.
        #[serde(rename = "style", skip_serializing_if = "Option::is_none")]
        pub style: Option<Style>,
        /// A unique identifier representing your end-user, which can help OpenAI to monitor and detect abuse. [Learn more](/docs/guides/safety-best-practices#end-user-ids).
        #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
        pub user: Option<String>,
    }

    impl CreateImageRequest {
        pub fn new(prompt: String) -> CreateImageRequest {
            CreateImageRequest {
                prompt,
                model: None,
                n: None,
                quality: None,
                response_format: None,
                output_format: None,
                output_compression: None,
                stream: None,
                partial_images: None,
                size: None,
                moderation: None,
                background: None,
                style: None,
                user: None,
            }
        }
    }
    /// The quality of the image that will be generated.  - `auto` (default value) will automatically select the best quality for the given model. - `high`, `medium` and `low` are supported for the GPT image models. - `hd` and `standard` are supported for `dall-e-3`. - `standard` is the only option for `dall-e-2`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Quality {
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "hd")]
        Hd,
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
            Self::Standard
        }
    }
    /// The format in which generated images with `dall-e-2` and `dall-e-3` are returned. Must be one of `url` or `b64_json`. URLs are only valid for 60 minutes after the image has been generated. This parameter isn't supported for the GPT image models, which always return base64-encoded images.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum ResponseFormat {
        #[serde(rename = "url")]
        Url,
        #[serde(rename = "b64_json")]
        B64Json,
    }

    impl Default for ResponseFormat {
        fn default() -> ResponseFormat {
            Self::Url
        }
    }
    /// The format in which the generated images are returned. This parameter is only supported for the GPT image models. Must be one of `png`, `jpeg`, or `webp`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum OutputFormat {
        #[serde(rename = "png")]
        Png,
        #[serde(rename = "jpeg")]
        Jpeg,
        #[serde(rename = "webp")]
        Webp,
    }

    impl Default for OutputFormat {
        fn default() -> OutputFormat {
            Self::Png
        }
    }
    /// Control the content-moderation level for images generated by the GPT image models. Must be either `low` for less restrictive filtering or `auto` (default value).
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Moderation {
        #[serde(rename = "low")]
        Low,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Moderation {
        fn default() -> Moderation {
            Self::Low
        }
    }
    /// Allows to set transparency for the background of the generated image(s). This parameter is only supported for the GPT image models. Must be one of `transparent`, `opaque` or `auto` (default value). When `auto` is used, the model will automatically determine the best background for the image.  If `transparent`, the output format needs to support transparency, so it should be set to either `png` (default value) or `webp`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Background {
        #[serde(rename = "transparent")]
        Transparent,
        #[serde(rename = "opaque")]
        Opaque,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Background {
        fn default() -> Background {
            Self::Transparent
        }
    }
    /// The style of the generated images. This parameter is only supported for `dall-e-3`. Must be one of `vivid` or `natural`. Vivid causes the model to lean towards generating hyper-real and dramatic images. Natural causes the model to produce more natural, less hyper-real looking images.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub enum Style {
        #[serde(rename = "vivid")]
        Vivid,
        #[serde(rename = "natural")]
        Natural,
    }

    impl Default for Style {
        fn default() -> Style {
            Self::Vivid
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageRequestModel {}

    impl CreateImageRequestModel {
        /// The model to use for image generation. One of `dall-e-2`, `dall-e-3`, or a GPT image model (`gpt-image-1`, `gpt-image-1-mini`, `gpt-image-1.5`). Defaults to `dall-e-2` unless a parameter specific to the GPT image models is used.
        pub fn new() -> CreateImageRequestModel {
            CreateImageRequestModel {}
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageRequestSize {}

    impl CreateImageRequestSize {
        /// The size of the generated images. For `gpt-image-2` and `gpt-image-2-2026-04-21`, arbitrary resolutions are supported as `WIDTHxHEIGHT` strings, for example `1536x864`. Width and height must both be divisible by 16 and the requested aspect ratio must be between 1:3 and 3:1. Resolutions above `2560x1440` are experimental, and the maximum supported resolution is `3840x2160`. The requested size must also satisfy the model's current pixel and edge limits. The standard sizes `1024x1024`, `1536x1024`, and `1024x1536` are supported by the GPT image models; `auto` is supported for models that allow automatic sizing. For `dall-e-2`, use one of `256x256`, `512x512`, or `1024x1024`. For `dall-e-3`, use one of `1024x1024`, `1792x1024`, or `1024x1792`.
        pub fn new() -> CreateImageRequestSize {
            CreateImageRequestSize {}
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateImageVariationRequestModel {}

    impl CreateImageVariationRequestModel {
        /// The model to use for image generation. Only `dall-e-2` is supported at this time.
        pub fn new() -> CreateImageVariationRequestModel {
            CreateImageVariationRequestModel {}
        }
    }
}

pub mod image_gen_completed_event {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenCompletedEvent {
        /// The type of the event. Always `image_generation.completed`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// Base64-encoded image data, suitable for rendering as an image.
        #[serde(rename = "b64_json")]
        pub b64_json: String,
        /// The Unix timestamp when the event was created.
        #[serde(rename = "created_at")]
        pub created_at: i32,
        /// The size of the generated image.
        #[serde(rename = "size")]
        pub size: Size,
        /// The quality setting for the generated image.
        #[serde(rename = "quality")]
        pub quality: Quality,
        /// The background setting for the generated image.
        #[serde(rename = "background")]
        pub background: Background,
        /// The output format for the generated image.
        #[serde(rename = "output_format")]
        pub output_format: OutputFormat,
        #[serde(rename = "usage")]
        pub usage: Box<models::ImagesUsage>,
    }

    impl ImageGenCompletedEvent {
        /// Emitted when image generation has completed and the final image is available.
        pub fn new(
            r#type: Type,
            b64_json: String,
            created_at: i32,
            size: Size,
            quality: Quality,
            background: Background,
            output_format: OutputFormat,
            usage: models::ImagesUsage,
        ) -> ImageGenCompletedEvent {
            ImageGenCompletedEvent {
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
    /// The type of the event. Always `image_generation.completed`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Type {
        #[serde(rename = "image_generation.completed")]
        ImageGenerationCompleted,
    }

    impl Default for Type {
        fn default() -> Type {
            Self::ImageGenerationCompleted
        }
    }
    /// The size of the generated image.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Size {
        #[serde(rename = "1024x1024")]
        Variant1024x1024,
        #[serde(rename = "1024x1536")]
        Variant1024x1536,
        #[serde(rename = "1536x1024")]
        Variant1536x1024,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Size {
        fn default() -> Size {
            Self::Variant1024x1024
        }
    }
    /// The quality setting for the generated image.
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
    /// The background setting for the generated image.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Background {
        #[serde(rename = "transparent")]
        Transparent,
        #[serde(rename = "opaque")]
        Opaque,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Background {
        fn default() -> Background {
            Self::Transparent
        }
    }
    /// The output format for the generated image.
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
}

pub mod image_gen_input_usage_details {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenInputUsageDetails {
        /// The number of text tokens in the input prompt.
        #[serde(rename = "text_tokens")]
        pub text_tokens: i32,
        /// The number of image tokens in the input prompt.
        #[serde(rename = "image_tokens")]
        pub image_tokens: i32,
    }

    impl ImageGenInputUsageDetails {
        /// The input tokens detailed information for the image generation.
        pub fn new(text_tokens: i32, image_tokens: i32) -> ImageGenInputUsageDetails {
            ImageGenInputUsageDetails { text_tokens, image_tokens }
        }
    }
}

pub mod image_gen_output_tokens_details {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenOutputTokensDetails {
        /// The number of image output tokens generated by the model.
        #[serde(rename = "image_tokens")]
        pub image_tokens: i32,
        /// The number of text output tokens generated by the model.
        #[serde(rename = "text_tokens")]
        pub text_tokens: i32,
    }

    impl ImageGenOutputTokensDetails {
        /// The output token details for the image generation.
        pub fn new(image_tokens: i32, text_tokens: i32) -> ImageGenOutputTokensDetails {
            ImageGenOutputTokensDetails { image_tokens, text_tokens }
        }
    }
}

pub mod image_gen_partial_image_event {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenPartialImageEvent {
        /// The type of the event. Always `image_generation.partial_image`.
        #[serde(rename = "type")]
        pub r#type: Type,
        /// Base64-encoded partial image data, suitable for rendering as an image.
        #[serde(rename = "b64_json")]
        pub b64_json: String,
        /// The Unix timestamp when the event was created.
        #[serde(rename = "created_at")]
        pub created_at: i32,
        /// The size of the requested image.
        #[serde(rename = "size")]
        pub size: Size,
        /// The quality setting for the requested image.
        #[serde(rename = "quality")]
        pub quality: Quality,
        /// The background setting for the requested image.
        #[serde(rename = "background")]
        pub background: Background,
        /// The output format for the requested image.
        #[serde(rename = "output_format")]
        pub output_format: OutputFormat,
        /// 0-based index for the partial image (streaming).
        #[serde(rename = "partial_image_index")]
        pub partial_image_index: i32,
    }

    impl ImageGenPartialImageEvent {
        /// Emitted when a partial image is available during image generation streaming.
        pub fn new(
            r#type: Type,
            b64_json: String,
            created_at: i32,
            size: Size,
            quality: Quality,
            background: Background,
            output_format: OutputFormat,
            partial_image_index: i32,
        ) -> ImageGenPartialImageEvent {
            ImageGenPartialImageEvent {
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
    /// The type of the event. Always `image_generation.partial_image`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Type {
        #[serde(rename = "image_generation.partial_image")]
        ImageGenerationPartialImage,
    }

    impl Default for Type {
        fn default() -> Type {
            Self::ImageGenerationPartialImage
        }
    }
    /// The size of the requested image.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Size {
        #[serde(rename = "1024x1024")]
        Variant1024x1024,
        #[serde(rename = "1024x1536")]
        Variant1024x1536,
        #[serde(rename = "1536x1024")]
        Variant1536x1024,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Size {
        fn default() -> Size {
            Self::Variant1024x1024
        }
    }
    /// The quality setting for the requested image.
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
    /// The background setting for the requested image.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Background {
        #[serde(rename = "transparent")]
        Transparent,
        #[serde(rename = "opaque")]
        Opaque,
        #[serde(rename = "auto")]
        Auto,
    }

    impl Default for Background {
        fn default() -> Background {
            Self::Transparent
        }
    }
    /// The output format for the requested image.
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
}

pub mod image_gen_stream_event {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum ImageGenStreamEvent {}
}

pub mod image_gen_usage {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImageGenUsage {
        /// The number of tokens (images and text) in the input prompt.
        #[serde(rename = "input_tokens")]
        pub input_tokens: i32,
        /// The total number of tokens (images and text) used for the image generation.
        #[serde(rename = "total_tokens")]
        pub total_tokens: i32,
        /// The number of output tokens generated by the model.
        #[serde(rename = "output_tokens")]
        pub output_tokens: i32,
        #[serde(rename = "input_tokens_details")]
        pub input_tokens_details: Box<models::ImageGenInputUsageDetails>,
        #[serde(rename = "output_tokens_details", skip_serializing_if = "Option::is_none")]
        pub output_tokens_details: Option<Box<models::ImageGenOutputTokensDetails>>,
    }

    impl ImageGenUsage {
        /// For `gpt-image-1` only, the token usage information for the image generation.
        pub fn new(
            input_tokens: i32,
            total_tokens: i32,
            output_tokens: i32,
            input_tokens_details: models::ImageGenInputUsageDetails,
        ) -> ImageGenUsage {
            ImageGenUsage {
                input_tokens,
                total_tokens,
                output_tokens,
                input_tokens_details: Box::new(input_tokens_details),
                output_tokens_details: None,
            }
        }
    }
}

pub(crate) use create::Background;
pub use create::CreateImageEditRequestImage;
pub use create::CreateImageEditRequestModel;
pub use create::CreateImageEditRequestSize;
pub use create::CreateImageRequest;
pub use create::CreateImageRequestModel;
pub use create::CreateImageRequestSize;
pub use create::CreateImageVariationRequestModel;
pub(crate) use create::Moderation;
pub(crate) use create::OutputFormat;
pub(crate) use create::Quality;
pub use create::ResponseFormat;
pub use create::Style;
pub use image_gen_completed_event::ImageGenCompletedEvent;
pub(crate) use image_gen_completed_event::Size;
pub(crate) use image_gen_completed_event::Type;
pub use image_gen_input_usage_details::ImageGenInputUsageDetails;
pub use image_gen_output_tokens_details::ImageGenOutputTokensDetails;
pub use image_gen_partial_image_event::ImageGenPartialImageEvent;
pub use image_gen_stream_event::ImageGenStreamEvent;
pub use image_gen_usage::ImageGenUsage;
