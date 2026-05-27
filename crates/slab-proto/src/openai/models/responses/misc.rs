use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseError {
    #[serde(rename = "code")]
    pub code: models::ResponseErrorCode,
    /// A human-readable description of the error.
    #[serde(rename = "message")]
    pub message: String,
}

impl ResponseError {
    /// An error object returned when the model fails to generate a Response.
    pub fn new(code: models::ResponseErrorCode, message: String) -> ResponseError {
        ResponseError { code, message }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseErrorCode {
    #[serde(rename = "server_error")]
    #[default]
    ServerError,
    #[serde(rename = "rate_limit_exceeded")]
    RateLimitExceeded,
    #[serde(rename = "invalid_prompt")]
    InvalidPrompt,
    #[serde(rename = "vector_store_timeout")]
    VectorStoreTimeout,
    #[serde(rename = "invalid_image")]
    InvalidImage,
    #[serde(rename = "invalid_image_format")]
    InvalidImageFormat,
    #[serde(rename = "invalid_base64_image")]
    InvalidBase64Image,
    #[serde(rename = "invalid_image_url")]
    InvalidImageUrl,
    #[serde(rename = "image_too_large")]
    ImageTooLarge,
    #[serde(rename = "image_too_small")]
    ImageTooSmall,
    #[serde(rename = "image_parse_error")]
    ImageParseError,
    #[serde(rename = "image_content_policy_violation")]
    ImageContentPolicyViolation,
    #[serde(rename = "invalid_image_mode")]
    InvalidImageMode,
    #[serde(rename = "image_file_too_large")]
    ImageFileTooLarge,
    #[serde(rename = "unsupported_image_media_type")]
    UnsupportedImageMediaType,
    #[serde(rename = "empty_image_file")]
    EmptyImageFile,
    #[serde(rename = "failed_to_download_image")]
    FailedToDownloadImage,
    #[serde(rename = "image_file_not_found")]
    ImageFileNotFound,
}

impl std::fmt::Display for ResponseErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ServerError => write!(f, "server_error"),
            Self::RateLimitExceeded => write!(f, "rate_limit_exceeded"),
            Self::InvalidPrompt => write!(f, "invalid_prompt"),
            Self::VectorStoreTimeout => write!(f, "vector_store_timeout"),
            Self::InvalidImage => write!(f, "invalid_image"),
            Self::InvalidImageFormat => write!(f, "invalid_image_format"),
            Self::InvalidBase64Image => write!(f, "invalid_base64_image"),
            Self::InvalidImageUrl => write!(f, "invalid_image_url"),
            Self::ImageTooLarge => write!(f, "image_too_large"),
            Self::ImageTooSmall => write!(f, "image_too_small"),
            Self::ImageParseError => write!(f, "image_parse_error"),
            Self::ImageContentPolicyViolation => write!(f, "image_content_policy_violation"),
            Self::InvalidImageMode => write!(f, "invalid_image_mode"),
            Self::ImageFileTooLarge => write!(f, "image_file_too_large"),
            Self::UnsupportedImageMediaType => write!(f, "unsupported_image_media_type"),
            Self::EmptyImageFile => write!(f, "empty_image_file"),
            Self::FailedToDownloadImage => write!(f, "failed_to_download_image"),
            Self::ImageFileNotFound => write!(f, "image_file_not_found"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseOutputText {
    /// Type discriminator that is always `output_text`.
    #[serde(rename = "type")]
    pub r#type: ResponseOutputTextType,
    /// Assistant generated text.
    #[serde(rename = "text")]
    pub text: String,
    /// Ordered list of annotations attached to the response text.
    #[serde(rename = "annotations")]
    pub annotations: Vec<models::ResponseOutputTextAnnotationsInner>,
}

impl ResponseOutputText {
    /// Assistant response text accompanied by optional annotations.
    pub fn new(
        r#type: ResponseOutputTextType,
        text: String,
        annotations: Vec<models::ResponseOutputTextAnnotationsInner>,
    ) -> ResponseOutputText {
        ResponseOutputText { r#type, text, annotations }
    }
}
/// Type discriminator that is always `output_text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseOutputTextType {
    #[serde(rename = "output_text")]
    #[default]
    OutputText,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseOutputTextAnnotationsInner {
    #[serde(rename = "FileAnnotation")]
    FileAnnotation(Box<models::FileAnnotation>),
    #[serde(rename = "UrlAnnotation")]
    UrlAnnotation(Box<models::UrlAnnotation>),
}

impl Default for ResponseOutputTextAnnotationsInner {
    fn default() -> Self {
        Self::FileAnnotation(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsePromptVariablesValue {
    String(String),
    InputTextContent(Box<models::InputTextContent>),
    InputImageContent(Box<models::InputImageContent>),
    InputFileContent(Box<models::InputFileContent>),
}

impl Default for ResponsePromptVariablesValue {
    fn default() -> Self {
        Self::String(Default::default())
    }
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseTextParam {
    #[serde(rename = "format", skip_serializing_if = "Option::is_none")]
    pub format: Option<Box<models::TextResponseFormatConfiguration>>,
    #[serde(
        rename = "verbosity",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub verbosity: Option<Option<models::Verbosity>>,
}

impl ResponseTextParam {
    /// Configuration options for a text response from the model. Can be plain text or structured JSON data. Learn more: - [Text inputs and outputs](/docs/guides/text) - [Structured Outputs](/docs/guides/structured-outputs)
    pub fn new() -> ResponseTextParam {
        ResponseTextParam { format: None, verbosity: None }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Reason {
    #[serde(rename = "max_output_tokens")]
    #[default]
    MaxOutputTokens,
    #[serde(rename = "content_filter")]
    ContentFilter,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ResponseStatus {
    #[serde(rename = "completed")]
    #[default]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "incomplete")]
    Incomplete,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseLogProb {
    /// A possible text token.
    #[serde(rename = "token")]
    pub token: String,
    /// The log probability of this token.
    #[serde(rename = "logprob")]
    pub logprob: f64,
    /// The log probabilities of up to 20 of the most likely tokens.
    #[serde(rename = "top_logprobs", skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<models::ResponseLogProbTopLogprobsInner>>,
}

impl ResponseLogProb {
    /// A logprob is the logarithmic probability that the model assigns to producing  a particular token at a given position in the sequence. Less-negative (higher)  logprob values indicate greater model confidence in that token choice.
    pub fn new(token: String, logprob: f64) -> ResponseLogProb {
        ResponseLogProb { token, logprob, top_logprobs: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseLogProbTopLogprobsInner {
    /// A possible text token.
    #[serde(rename = "token", skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The log probability of this token.
    #[serde(rename = "logprob", skip_serializing_if = "Option::is_none")]
    pub logprob: Option<f64>,
}

impl ResponseLogProbTopLogprobsInner {
    pub fn new() -> ResponseLogProbTopLogprobsInner {
        ResponseLogProbTopLogprobsInner { token: None, logprob: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseUsage {
    /// The number of input tokens.
    #[serde(rename = "input_tokens")]
    pub input_tokens: i32,
    #[serde(rename = "input_tokens_details")]
    pub input_tokens_details: Box<models::ResponseUsageInputTokensDetails>,
    /// The number of output tokens.
    #[serde(rename = "output_tokens")]
    pub output_tokens: i32,
    #[serde(rename = "output_tokens_details")]
    pub output_tokens_details: Box<models::ResponseUsageOutputTokensDetails>,
    /// The total number of tokens used.
    #[serde(rename = "total_tokens")]
    pub total_tokens: i32,
}

impl ResponseUsage {
    /// Represents token usage details including input tokens, output tokens, a breakdown of output tokens, and the total tokens used.
    pub fn new(
        input_tokens: i32,
        input_tokens_details: models::ResponseUsageInputTokensDetails,
        output_tokens: i32,
        output_tokens_details: models::ResponseUsageOutputTokensDetails,
        total_tokens: i32,
    ) -> ResponseUsage {
        ResponseUsage {
            input_tokens,
            input_tokens_details: Box::new(input_tokens_details),
            output_tokens,
            output_tokens_details: Box::new(output_tokens_details),
            total_tokens,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseUsageInputTokensDetails {
    /// The number of tokens that were retrieved from the cache.  [More on prompt caching](/docs/guides/prompt-caching).
    #[serde(rename = "cached_tokens")]
    pub cached_tokens: i32,
}

impl ResponseUsageInputTokensDetails {
    /// A detailed breakdown of the input tokens.
    pub fn new(cached_tokens: i32) -> ResponseUsageInputTokensDetails {
        ResponseUsageInputTokensDetails { cached_tokens }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponseUsageOutputTokensDetails {
    /// The number of reasoning tokens.
    #[serde(rename = "reasoning_tokens")]
    pub reasoning_tokens: i32,
}

impl ResponseUsageOutputTokensDetails {
    /// A detailed breakdown of the output tokens.
    pub fn new(reasoning_tokens: i32) -> ResponseUsageOutputTokensDetails {
        ResponseUsageOutputTokensDetails { reasoning_tokens }
    }
}
