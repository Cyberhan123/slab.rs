use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageParamsModeration {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "low")]
    Low,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ImageParamsStatus {
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

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
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
