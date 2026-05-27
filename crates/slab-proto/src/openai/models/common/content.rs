use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArrayOfContentPartsInner {
    MessageContentImageFileObject(Box<models::MessageContentImageFileObject>),
    MessageContentImageUrlObject(Box<models::MessageContentImageUrlObject>),
    MessageRequestContentTextObject(Box<models::MessageRequestContentTextObject>),
}

impl Default for ArrayOfContentPartsInner {
    fn default() -> Self {
        Self::MessageContentImageFileObject(Default::default())
    }
}
/// Always `image_file`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ArrayOfContentPartsInnerType {
    #[serde(rename = "image_file")]
    ImageFile,
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(rename = "text")]
    Text,
}

impl Default for ArrayOfContentPartsInnerType {
    fn default() -> ArrayOfContentPartsInnerType {
        Self::ImageFile
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    InputContentTypes(Box<models::InputContent>),
    OutputContentTypes(Box<models::OutputContent>),
}

impl Default for Content {
    fn default() -> Self {
        Self::InputContentTypes(Default::default())
    }
}
/// The type of the reasoning text. Always `reasoning_text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum ContentType {
    #[serde(rename = "reasoning_text")]
    ReasoningText,
}

impl Default for ContentType {
    fn default() -> ContentType {
        Self::ReasoningText
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub r#type: TextContentType,
    #[serde(rename = "text")]
    pub text: String,
}

impl TextContent {
    /// A text content.
    pub fn new(r#type: TextContentType, text: String) -> TextContent {
        TextContent { r#type, text }
    }
}

/// The type of the text content. Always `text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum TextContentType {
    #[serde(rename = "text")]
    Text,
}

impl Default for TextContentType {
    fn default() -> TextContentType {
        Self::Text
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputTextContent {
    /// The type of the output text. Always `output_text`.
    #[serde(rename = "type")]
    pub r#type: OutputTextContentType,
    /// The text output from the model.
    #[serde(rename = "text")]
    pub text: String,
    /// The annotations of the text output.
    #[serde(rename = "annotations")]
    pub annotations: Vec<models::Annotation>,
    #[serde(rename = "logprobs")]
    pub logprobs: Vec<models::LogProb>,
}

impl OutputTextContent {
    /// A text output from the model.
    pub fn new(
        r#type: OutputTextContentType,
        text: String,
        annotations: Vec<models::Annotation>,
        logprobs: Vec<models::LogProb>,
    ) -> OutputTextContent {
        OutputTextContent { r#type, text, annotations, logprobs }
    }
}
/// The type of the output text. Always `output_text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum OutputTextContentType {
    #[serde(rename = "output_text")]
    OutputText,
}

impl Default for OutputTextContentType {
    fn default() -> OutputTextContentType {
        Self::OutputText
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryTextContent {
    /// The type of the object. Always `summary_text`.
    #[serde(rename = "type")]
    pub r#type: SummaryTextContentType,
    /// A summary of the reasoning output from the model so far.
    #[serde(rename = "text")]
    pub text: String,
}

impl SummaryTextContent {
    /// A summary text from the model.
    pub fn new(r#type: SummaryTextContentType, text: String) -> SummaryTextContent {
        SummaryTextContent { r#type, text }
    }
}
/// The type of the object. Always `summary_text`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum SummaryTextContentType {
    #[serde(rename = "summary_text")]
    SummaryText,
}

impl Default for SummaryTextContentType {
    fn default() -> SummaryTextContentType {
        Self::SummaryText
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefusalContent {
    /// The type of the refusal. Always `refusal`.
    #[serde(rename = "type")]
    pub r#type: RefusalContentType,
    /// The refusal explanation from the model.
    #[serde(rename = "refusal")]
    pub refusal: String,
}

impl RefusalContent {
    /// A refusal from the model.
    pub fn new(r#type: RefusalContentType, refusal: String) -> RefusalContent {
        RefusalContent { r#type, refusal }
    }
}
/// The type of the refusal. Always `refusal`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum RefusalContentType {
    #[serde(rename = "refusal")]
    Refusal,
}

impl Default for RefusalContentType {
    fn default() -> RefusalContentType {
        Self::Refusal
    }
}
