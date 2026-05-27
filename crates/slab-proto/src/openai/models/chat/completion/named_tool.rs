use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionNamedToolChoiceCustomCustom {
    /// The name of the custom tool to call.
    #[serde(rename = "name")]
    pub name: String,
}

impl ChatCompletionNamedToolChoiceCustomCustom {
    pub fn new(name: String) -> ChatCompletionNamedToolChoiceCustomCustom {
        ChatCompletionNamedToolChoiceCustomCustom { name }
    }
}

/// ChatCompletionNamedToolChoiceCustom : Specifies a tool the model should use. Use to force the model to call a specific custom tool.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionNamedToolChoiceCustom {
    /// For custom tool calling, the type is always `custom`.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionNamedToolChoiceCustomType,
    #[serde(rename = "custom")]
    pub custom: Box<models::ChatCompletionNamedToolChoiceCustomCustom>,
}

impl ChatCompletionNamedToolChoiceCustom {
    /// Specifies a tool the model should use. Use to force the model to call a specific custom tool.
    pub fn new(
        r#type: ChatCompletionNamedToolChoiceCustomType,
        custom: models::ChatCompletionNamedToolChoiceCustomCustom,
    ) -> ChatCompletionNamedToolChoiceCustom {
        ChatCompletionNamedToolChoiceCustom { r#type, custom: Box::new(custom) }
    }
}
/// For custom tool calling, the type is always `custom`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ChatCompletionNamedToolChoiceCustomType {
    #[serde(rename = "custom")]
    #[default]
    Custom,
}

/// ChatCompletionNamedToolChoice : Specifies a tool the model should use. Use to force the model to call a specific function.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionNamedToolChoice {
    /// For function calling, the type is always `function`.
    #[serde(rename = "type")]
    pub r#type: ChatCompletionNamedToolChoiceType,
    #[serde(rename = "function")]
    pub function: Box<models::AssistantsNamedToolChoiceFunction>,
}

impl ChatCompletionNamedToolChoice {
    /// Specifies a tool the model should use. Use to force the model to call a specific function.
    pub fn new(
        r#type: ChatCompletionNamedToolChoiceType,
        function: models::AssistantsNamedToolChoiceFunction,
    ) -> ChatCompletionNamedToolChoice {
        ChatCompletionNamedToolChoice { r#type, function: Box::new(function) }
    }
}
/// For function calling, the type is always `function`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ChatCompletionNamedToolChoiceType {
    #[serde(rename = "function")]
    #[default]
    Function,
}
