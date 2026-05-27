use crate::models;
use serde::{Deserialize, Serialize};

/// ChatCompletionToolChoiceOption : Controls which (if any) tool is called by the model. `none` means the model will not call any tool and instead generates a message. `auto` means the model can pick between generating a message or calling one or more tools. `required` means the model must call one or more tools. Specifying a particular tool via `{\"type\": \"function\", \"function\": {\"name\": \"my_function\"}}` forces the model to call that tool.  `none` is the default when no tools are present. `auto` is the default if tools are present.
/// Controls which (if any) tool is called by the model. `none` means the model will not call any tool and instead generates a message. `auto` means the model can pick between generating a message or calling one or more tools. `required` means the model must call one or more tools. Specifying a particular tool via `{\"type\": \"function\", \"function\": {\"name\": \"my_function\"}}` forces the model to call that tool.  `none` is the default when no tools are present. `auto` is the default if tools are present.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionToolChoiceOption {
    /// `none` means the model will not call any tool and instead generates a message. `auto` means the model can pick between generating a message or calling one or more tools. `required` means the model must call one or more tools.
    ToolChoiceMode(String),
    ChatCompletionAllowedToolsChoice(Box<models::ChatCompletionAllowedToolsChoice>),
    ChatCompletionNamedToolChoice(Box<models::ChatCompletionNamedToolChoice>),
    ChatCompletionNamedToolChoiceCustom(Box<models::ChatCompletionNamedToolChoiceCustom>),
}

impl Default for ChatCompletionToolChoiceOption {
    fn default() -> Self {
        Self::ToolChoiceMode(Default::default())
    }
}
/// Allowed tool configuration type. Always `allowed_tools`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ChatCompletionToolChoiceOptionType {
    #[serde(rename = "allowed_tools")]
    #[default]
    AllowedTools,
    #[serde(rename = "function")]
    Function,
    #[serde(rename = "custom")]
    Custom,
}

/// ChatCompletionRequestToolMessageContent : The contents of the tool message.
/// The contents of the tool message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatCompletionRequestToolMessageContent {
    /// The contents of the tool message.
    TextContent(String),
    /// An array of content parts with a defined type. For tool messages, only type `text` is supported.
    ArrayContentParts(Vec<models::ChatCompletionRequestMessageContentPartText>),
}

impl Default for ChatCompletionRequestToolMessageContent {
    fn default() -> Self {
        Self::TextContent(Default::default())
    }
}

/// ChatCompletionTool : A function tool that can be used to generate a response.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionTool {
    /// The type of the tool. Currently, only `function` is supported.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionToolType,
    #[serde(rename = "function")]
    pub function: Box<models::FunctionObject>,
}

impl ChatCompletionTool {
    /// A function tool that can be used to generate a response.
    pub fn new(
        r#type: ChatCompletionToolType,
        function: models::FunctionObject,
    ) -> ChatCompletionTool {
        ChatCompletionTool { r#type, function: Box::new(function) }
    }
}
/// The type of the tool. Currently, only `function` is supported.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ChatCompletionToolType {
    #[serde(rename = "function")]
    #[default]
    Function,
}
