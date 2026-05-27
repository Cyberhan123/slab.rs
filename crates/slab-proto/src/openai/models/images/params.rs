pub mod output_format {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum OutputFormat {
        #[serde(rename = "png")]
        #[default]
        Png,
        #[serde(rename = "webp")]
        Webp,
        #[serde(rename = "jpeg")]
        Jpeg,
    }

    
    // The size of the image generated. Either `1024x1024`, `1024x1536`, or `1536x1024`.
}

pub mod quality {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Quality {
        #[serde(rename = "low")]
        #[default]
        Low,
        #[serde(rename = "medium")]
        Medium,
        #[serde(rename = "high")]
        High,
    }

    
}

pub mod size {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Size {
        #[serde(rename = "1024x1024")]
        #[default]
        Variant1024x1024,
        #[serde(rename = "1024x1536")]
        Variant1024x1536,
        #[serde(rename = "1536x1024")]
        Variant1536x1024,
    }

    
    // The quality of the image generated. Either `low`, `medium`, or `high`.
}

pub mod moderation {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Moderation {
        #[serde(rename = "low")]
        #[default]
        Low,
        #[serde(rename = "auto")]
        Auto,
    }

    
    /// Background behavior for generated image output.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Background {
        #[serde(rename = "transparent")]
        #[default]
        Transparent,
        #[serde(rename = "opaque")]
        Opaque,
        #[serde(rename = "auto")]
        Auto,
    }

    
}

pub mod moderation_1 {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Moderation {
        #[serde(rename = "auto")]
        #[default]
        Auto,
        #[serde(rename = "low")]
        Low,
    }

    
    /// Background type for the generated image. One of `transparent`, `opaque`, or `auto`. Default: `auto`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub(crate) enum Background {
        #[serde(rename = "transparent")]
        #[default]
        Transparent,
        #[serde(rename = "opaque")]
        Opaque,
        #[serde(rename = "auto")]
        Auto,
    }

    
}

pub mod status {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum Status {
        #[serde(rename = "in_progress")]
        #[default]
        InProgress,
        #[serde(rename = "completed")]
        Completed,
        #[serde(rename = "generating")]
        Generating,
        #[serde(rename = "failed")]
        Failed,
    }

    
}

pub mod usage {
    /*
     * OpenAI API - Merged type definitions
     */

    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImagesUsage {
        /// The total number of tokens (images and text) used for the image generation.
        #[serde(rename = "total_tokens")]
        pub total_tokens: i32,
        /// The number of tokens (images and text) in the input prompt.
        #[serde(rename = "input_tokens")]
        pub input_tokens: i32,
        /// The number of image tokens in the output image.
        #[serde(rename = "output_tokens")]
        pub output_tokens: i32,
        #[serde(rename = "input_tokens_details")]
        pub input_tokens_details: Box<models::ImagesUsageInputTokensDetails>,
    }

    impl ImagesUsage {
        /// For the GPT image models only, the token usage information for the image generation.
        pub fn new(
            total_tokens: i32,
            input_tokens: i32,
            output_tokens: i32,
            input_tokens_details: models::ImagesUsageInputTokensDetails,
        ) -> ImagesUsage {
            ImagesUsage {
                total_tokens,
                input_tokens,
                output_tokens,
                input_tokens_details: Box::new(input_tokens_details),
            }
        }
    }

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct ImagesUsageInputTokensDetails {
        /// The number of text tokens in the input prompt.
        #[serde(rename = "text_tokens")]
        pub text_tokens: i32,
        /// The number of image tokens in the input prompt.
        #[serde(rename = "image_tokens")]
        pub image_tokens: i32,
    }

    impl ImagesUsageInputTokensDetails {
        /// The input tokens detailed information for the image generation.
        pub fn new(text_tokens: i32, image_tokens: i32) -> ImagesUsageInputTokensDetails {
            ImagesUsageInputTokensDetails { text_tokens, image_tokens }
        }
    }
}

pub mod image_gen_action_enum {
    
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    #[derive(Default)]
    pub enum ImageGenActionEnum {
        #[serde(rename = "generate")]
        #[default]
        Generate,
        #[serde(rename = "edit")]
        Edit,
        #[serde(rename = "auto")]
        Auto,
    }

    impl std::fmt::Display for ImageGenActionEnum {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                Self::Generate => write!(f, "generate"),
                Self::Edit => write!(f, "edit"),
                Self::Auto => write!(f, "auto"),
            }
        }
    }

    
}

pub use image_gen_action_enum::ImageGenActionEnum;
pub(crate) use moderation::Moderation;
pub use status::Status;
pub use usage::ImagesUsage;
pub use usage::ImagesUsageInputTokensDetails;
