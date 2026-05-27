use crate::models;
use serde::{Deserialize, Serialize};

/// ChatCompletionAllowedToolsChoice : Constrains the tools available to the model to a pre-defined set.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionAllowedToolsChoice {
    /// Allowed tool configuration type. Always `allowed_tools`.
    #[serde(rename = "type")]
    pub r#type: Type,
    #[serde(rename = "allowed_tools")]
    pub allowed_tools: Box<models::ChatCompletionAllowedTools>,
}

impl ChatCompletionAllowedToolsChoice {
    /// Constrains the tools available to the model to a pre-defined set.
    pub fn new(
        r#type: Type,
        allowed_tools: models::ChatCompletionAllowedTools,
    ) -> ChatCompletionAllowedToolsChoice {
        ChatCompletionAllowedToolsChoice { r#type, allowed_tools: Box::new(allowed_tools) }
    }
}
/// Allowed tool configuration type. Always `allowed_tools`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Type {
    #[serde(rename = "allowed_tools")]
    #[default]
    AllowedTools,
}


/// ChatCompletionAllowedTools : Constrains the tools available to the model to a pre-defined set.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionAllowedTools {
    /// Constrains the tools available to the model to a pre-defined set.  `auto` allows the model to pick from among the allowed tools and generate a message.  `required` requires the model to call one or more of the allowed tools.
    #[serde(rename = "mode")]
    pub mode: Mode,
    /// A list of tool definitions that the model should be allowed to call.  For the Chat Completions API, the list of tool definitions might look like: ```json [   { \"type\": \"function\", \"function\": { \"name\": \"get_weather\" } },   { \"type\": \"function\", \"function\": { \"name\": \"get_time\" } } ] ```
    #[serde(rename = "tools")]
    pub tools: Vec<std::collections::HashMap<String, serde_json::Value>>,
}

impl ChatCompletionAllowedTools {
    /// Constrains the tools available to the model to a pre-defined set.
    pub fn new(
        mode: Mode,
        tools: Vec<std::collections::HashMap<String, serde_json::Value>>,
    ) -> ChatCompletionAllowedTools {
        ChatCompletionAllowedTools { mode, tools }
    }
}
/// Constrains the tools available to the model to a pre-defined set.  `auto` allows the model to pick from among the allowed tools and generate a message.  `required` requires the model to call one or more of the allowed tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Mode {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "required")]
    Required,
}

