use crate::models;
use serde::{Deserialize, Serialize};

/// ChatCompletionRequestSystemMessageContent : The contents of the system message.
/// The contents of the system message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionRequestSystemMessageContent {
    /// The contents of the system message.
    TextContent(String),
    /// An array of content parts with a defined type. For system messages, only type `text` is supported.
    ArrayContentParts(Vec<models::ChatCompletionRequestMessageContentPartText>),
}

impl Default for ChatCompletionRequestSystemMessageContent {
    fn default() -> Self {
        Self::TextContent(Default::default())
    }
}

/// ChatCompletionRequestSystemMessage : Developer-provided instructions that the model should follow, regardless of messages sent by the user. With o1 models and newer, use `developer` messages for this purpose instead.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestSystemMessage {
    #[serde(rename = "content")]
    pub content: Box<models::ChatCompletionRequestSystemMessageContent>,
    /// The role of the messages author, in this case `system`.
    #[serde(rename = "role")]
    pub role: ChatCompletionRequestSystemMessageRole,
    /// An optional name for the participant. Provides the model information to differentiate between participants of the same role.
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatCompletionRequestSystemMessage {
    /// Developer-provided instructions that the model should follow, regardless of messages sent by the user. With o1 models and newer, use `developer` messages for this purpose instead.
    pub fn new(
        content: models::ChatCompletionRequestSystemMessageContent,
        role: ChatCompletionRequestSystemMessageRole,
    ) -> ChatCompletionRequestSystemMessage {
        ChatCompletionRequestSystemMessage { content: Box::new(content), role, name: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestToolMessage {
    /// The role of the messages author, in this case `tool`.
    #[serde(rename = "role")]
    pub role: ChatCompletionRequestToolMessageRole,
    #[serde(rename = "content")]
    pub content: Box<models::ChatCompletionRequestToolMessageContent>,
    /// Tool call that this message is responding to.
    #[serde(rename = "tool_call_id")]
    pub tool_call_id: String,
}

impl ChatCompletionRequestToolMessage {
    pub fn new(
        role: ChatCompletionRequestToolMessageRole,
        content: models::ChatCompletionRequestToolMessageContent,
        tool_call_id: String,
    ) -> ChatCompletionRequestToolMessage {
        ChatCompletionRequestToolMessage { role, content: Box::new(content), tool_call_id }
    }
}
/// The role of the messages author, in this case `tool`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestToolMessageRole {
    #[serde(rename = "tool")]
    Tool,
}

impl Default for ChatCompletionRequestToolMessageRole {
    fn default() -> ChatCompletionRequestToolMessageRole {
        Self::Tool
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionRequestUserMessageContentPart {
    ChatCompletionRequestMessageContentPartText(
        Box<models::ChatCompletionRequestMessageContentPartText>,
    ),
    ChatCompletionRequestMessageContentPartImage(
        Box<models::ChatCompletionRequestMessageContentPartImage>,
    ),
    ChatCompletionRequestMessageContentPartAudio(
        Box<models::ChatCompletionRequestMessageContentPartAudio>,
    ),
    ChatCompletionRequestMessageContentPartFile(
        Box<models::ChatCompletionRequestMessageContentPartFile>,
    ),
}

impl Default for ChatCompletionRequestUserMessageContentPart {
    fn default() -> Self {
        Self::ChatCompletionRequestMessageContentPartText(Default::default())
    }
}
/// The type of the content part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestUserMessageContentPartType {
    #[serde(rename = "text")]
    Text,
    #[serde(rename = "image_url")]
    ImageUrl,
    #[serde(rename = "input_audio")]
    InputAudio,
    #[serde(rename = "file")]
    File,
}

impl Default for ChatCompletionRequestUserMessageContentPartType {
    fn default() -> ChatCompletionRequestUserMessageContentPartType {
        Self::Text
    }
}

/// ChatCompletionRequestUserMessageContent : The contents of the user message.
/// The contents of the user message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionRequestUserMessageContent {
    /// The text contents of the message.
    TextContent(String),
    /// An array of content parts with a defined type. Supported options differ based on the [model](/docs/models) being used to generate the response. Can contain text, image, or audio inputs.
    ArrayContentParts(Vec<models::ChatCompletionRequestUserMessageContentPart>),
}

impl Default for ChatCompletionRequestUserMessageContent {
    fn default() -> Self {
        Self::TextContent(Default::default())
    }
}

/// ChatCompletionRequestUserMessage : Messages sent by an end user, containing prompts or additional context information.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestUserMessage {
    #[serde(rename = "content")]
    pub content: Box<models::ChatCompletionRequestUserMessageContent>,
    /// The role of the messages author, in this case `user`.
    #[serde(rename = "role")]
    pub role: ChatCompletionRequestUserMessageRole,
    /// An optional name for the participant. Provides the model information to differentiate between participants of the same role.
    #[serde(rename = "name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatCompletionRequestUserMessage {
    /// Messages sent by an end user, containing prompts or additional context information.
    pub fn new(
        content: models::ChatCompletionRequestUserMessageContent,
        role: ChatCompletionRequestUserMessageRole,
    ) -> ChatCompletionRequestUserMessage {
        ChatCompletionRequestUserMessage { content: Box::new(content), role, name: None }
    }
}
/// The role of the messages author, in this case `user`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestUserMessageRole {
    #[serde(rename = "user")]
    User,
}

impl Default for ChatCompletionRequestUserMessageRole {
    fn default() -> ChatCompletionRequestUserMessageRole {
        Self::User
    }
}

/// The role of the messages author, in this case `system`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestSystemMessageRole {
    #[serde(rename = "system")]
    System,
}

impl Default for ChatCompletionRequestSystemMessageRole {
    fn default() -> ChatCompletionRequestSystemMessageRole {
        Self::System
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestFunctionMessage {
    /// The role of the messages author, in this case `function`.
    #[serde(rename = "role")]
    pub role: ChatCompletionRequestFunctionMessageRole,
    /// The contents of the function message.
    #[serde(rename = "content", deserialize_with = "Option::deserialize")]
    pub content: Option<String>,
    /// The name of the function to call.
    #[serde(rename = "name")]
    pub name: String,
}

impl ChatCompletionRequestFunctionMessage {
    pub fn new(
        role: ChatCompletionRequestFunctionMessageRole,
        content: Option<String>,
        name: String,
    ) -> ChatCompletionRequestFunctionMessage {
        ChatCompletionRequestFunctionMessage { role, content, name }
    }
}
/// The role of the messages author, in this case `function`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestFunctionMessageRole {
    #[serde(rename = "function")]
    Function,
}

impl Default for ChatCompletionRequestFunctionMessageRole {
    fn default() -> ChatCompletionRequestFunctionMessageRole {
        Self::Function
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartAudioInputAudio {
    /// Base64 encoded audio data.
    #[serde(rename = "data")]
    pub data: String,
    /// The format of the encoded audio data. Currently supports \"wav\" and \"mp3\".
    #[serde(rename = "format")]
    pub format: Format,
}

impl ChatCompletionRequestMessageContentPartAudioInputAudio {
    pub fn new(
        data: String,
        format: Format,
    ) -> ChatCompletionRequestMessageContentPartAudioInputAudio {
        ChatCompletionRequestMessageContentPartAudioInputAudio { data, format }
    }
}
/// The format of the encoded audio data. Currently supports \"wav\" and \"mp3\".
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Format {
    #[serde(rename = "wav")]
    Wav,
    #[serde(rename = "mp3")]
    Mp3,
}

impl Default for Format {
    fn default() -> Format {
        Self::Wav
    }
}

/// ChatCompletionRequestMessageContentPartAudio : Learn about [audio inputs](/docs/guides/audio).
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartAudio {
    /// The type of the content part. Always `input_audio`.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionRequestMessageContentPartAudioType,
    #[serde(rename = "input_audio")]
    pub input_audio: Box<models::ChatCompletionRequestMessageContentPartAudioInputAudio>,
}

impl ChatCompletionRequestMessageContentPartAudio {
    /// Learn about [audio inputs](/docs/guides/audio).
    pub fn new(
        r#type: ChatCompletionRequestMessageContentPartAudioType,
        input_audio: models::ChatCompletionRequestMessageContentPartAudioInputAudio,
    ) -> ChatCompletionRequestMessageContentPartAudio {
        ChatCompletionRequestMessageContentPartAudio { r#type, input_audio: Box::new(input_audio) }
    }
}
/// The type of the content part. Always `input_audio`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestMessageContentPartAudioType {
    #[serde(rename = "input_audio")]
    InputAudio,
}

impl Default for ChatCompletionRequestMessageContentPartAudioType {
    fn default() -> ChatCompletionRequestMessageContentPartAudioType {
        Self::InputAudio
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartFileFile {
    /// The name of the file, used when passing the file to the model as a  string.
    #[serde(rename = "filename", skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// The base64 encoded file data, used when passing the file to the model  as a string.
    #[serde(rename = "file_data", skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    /// The ID of an uploaded file to use as input.
    #[serde(rename = "file_id", skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

impl ChatCompletionRequestMessageContentPartFileFile {
    pub fn new() -> ChatCompletionRequestMessageContentPartFileFile {
        ChatCompletionRequestMessageContentPartFileFile {
            filename: None,
            file_data: None,
            file_id: None,
        }
    }
}

/// ChatCompletionRequestMessageContentPartFile : Learn about [file inputs](/docs/guides/text) for text generation.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartFile {
    /// The type of the content part. Always `file`.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionRequestMessageContentPartFileType,
    #[serde(rename = "file")]
    pub file: Box<models::ChatCompletionRequestMessageContentPartFileFile>,
}

impl ChatCompletionRequestMessageContentPartFile {
    /// Learn about [file inputs](/docs/guides/text) for text generation.
    pub fn new(
        r#type: ChatCompletionRequestMessageContentPartFileType,
        file: models::ChatCompletionRequestMessageContentPartFileFile,
    ) -> ChatCompletionRequestMessageContentPartFile {
        ChatCompletionRequestMessageContentPartFile { r#type, file: Box::new(file) }
    }
}
/// The type of the content part. Always `file`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestMessageContentPartFileType {
    #[serde(rename = "file")]
    File,
}

impl Default for ChatCompletionRequestMessageContentPartFileType {
    fn default() -> ChatCompletionRequestMessageContentPartFileType {
        Self::File
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartImageImageUrl {
    /// Either a URL of the image or the base64 encoded image data.
    #[serde(rename = "url")]
    pub url: String,
    /// Specifies the detail level of the image. Learn more in the [Vision guide](/docs/guides/vision#low-or-high-fidelity-image-understanding).
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    pub detail: Option<Detail>,
}

impl ChatCompletionRequestMessageContentPartImageImageUrl {
    pub fn new(url: String) -> ChatCompletionRequestMessageContentPartImageImageUrl {
        ChatCompletionRequestMessageContentPartImageImageUrl { url, detail: None }
    }
}
/// Specifies the detail level of the image. Learn more in the [Vision guide](/docs/guides/vision#low-or-high-fidelity-image-understanding).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Detail {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "high")]
    High,
}

impl Default for Detail {
    fn default() -> Detail {
        Self::Auto
    }
}

/// ChatCompletionRequestMessageContentPartImage : Learn about [image inputs](/docs/guides/vision).
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartImage {
    /// The type of the content part.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionRequestMessageContentPartImageType,
    #[serde(rename = "image_url")]
    pub image_url: Box<models::ChatCompletionRequestMessageContentPartImageImageUrl>,
}

impl ChatCompletionRequestMessageContentPartImage {
    /// Learn about [image inputs](/docs/guides/vision).
    pub fn new(
        r#type: ChatCompletionRequestMessageContentPartImageType,
        image_url: models::ChatCompletionRequestMessageContentPartImageImageUrl,
    ) -> ChatCompletionRequestMessageContentPartImage {
        ChatCompletionRequestMessageContentPartImage { r#type, image_url: Box::new(image_url) }
    }
}
/// The type of the content part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestMessageContentPartImageType {
    #[serde(rename = "image_url")]
    ImageUrl,
}

impl Default for ChatCompletionRequestMessageContentPartImageType {
    fn default() -> ChatCompletionRequestMessageContentPartImageType {
        Self::ImageUrl
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartRefusal {
    /// The type of the content part.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionRequestMessageContentPartRefusalType,
    /// The refusal message generated by the model.
    #[serde(rename = "refusal")]
    pub refusal: String,
}

impl ChatCompletionRequestMessageContentPartRefusal {
    pub fn new(
        r#type: ChatCompletionRequestMessageContentPartRefusalType,
        refusal: String,
    ) -> ChatCompletionRequestMessageContentPartRefusal {
        ChatCompletionRequestMessageContentPartRefusal { r#type, refusal }
    }
}
/// The type of the content part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestMessageContentPartRefusalType {
    #[serde(rename = "refusal")]
    Refusal,
}

impl Default for ChatCompletionRequestMessageContentPartRefusalType {
    fn default() -> ChatCompletionRequestMessageContentPartRefusalType {
        Self::Refusal
    }
}

/// ChatCompletionRequestMessageContentPartText : Learn about [text inputs](/docs/guides/text-generation).
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestMessageContentPartText {
    /// The type of the content part.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionRequestMessageContentPartTextType,
    /// The text content.
    #[serde(rename = "text")]
    pub text: String,
}

impl ChatCompletionRequestMessageContentPartText {
    /// Learn about [text inputs](/docs/guides/text-generation).
    pub fn new(
        r#type: ChatCompletionRequestMessageContentPartTextType,
        text: String,
    ) -> ChatCompletionRequestMessageContentPartText {
        ChatCompletionRequestMessageContentPartText { r#type, text }
    }
}
/// The type of the content part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum ChatCompletionRequestMessageContentPartTextType {
    #[serde(rename = "text")]
    Text,
}

impl Default for ChatCompletionRequestMessageContentPartTextType {
    fn default() -> ChatCompletionRequestMessageContentPartTextType {
        Self::Text
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum ChatCompletionRequestMessage {
    #[serde(rename = "ChatCompletionRequestDeveloperMessage")]
    ChatCompletionRequestDeveloperMessage(Box<models::ChatCompletionRequestDeveloperMessage>),
    #[serde(rename = "ChatCompletionRequestSystemMessage")]
    ChatCompletionRequestSystemMessage(Box<models::ChatCompletionRequestSystemMessage>),
    #[serde(rename = "ChatCompletionRequestUserMessage")]
    ChatCompletionRequestUserMessage(Box<models::ChatCompletionRequestUserMessage>),
    #[serde(rename = "ChatCompletionRequestAssistantMessage")]
    ChatCompletionRequestAssistantMessage(Box<models::ChatCompletionRequestAssistantMessage>),
    #[serde(rename = "ChatCompletionRequestToolMessage")]
    ChatCompletionRequestToolMessage(Box<models::ChatCompletionRequestToolMessage>),
    #[serde(rename = "ChatCompletionRequestFunctionMessage")]
    ChatCompletionRequestFunctionMessage(Box<models::ChatCompletionRequestFunctionMessage>),
}

impl Default for ChatCompletionRequestMessage {
    fn default() -> Self {
        Self::ChatCompletionRequestDeveloperMessage(Default::default())
    }
}
