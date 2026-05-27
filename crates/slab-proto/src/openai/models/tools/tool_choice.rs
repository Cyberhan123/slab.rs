use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoice {
    /// Identifier of the requested tool.
    #[serde(rename = "id")]
    pub id: String,
}

impl ToolChoice {
    /// Tool selection that the assistant should honor when executing the item.
    pub fn new(id: String) -> ToolChoice {
        ToolChoice { id }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceAllowed {
    /// Allowed tool configuration type. Always `allowed_tools`.
    #[serde(rename = "type")]
    pub r#type: ToolChoiceAllowedType,
    /// Constrains the tools available to the model to a pre-defined set.  `auto` allows the model to pick from among the allowed tools and generate a message.  `required` requires the model to call one or more of the allowed tools.
    #[serde(rename = "mode")]
    pub mode: ToolChoiceAllowedMode,
    /// A list of tool definitions that the model should be allowed to call.  For the Responses API, the list of tool definitions might look like: ```json [   { \"type\": \"function\", \"name\": \"get_weather\" },   { \"type\": \"mcp\", \"server_label\": \"deepwiki\" },   { \"type\": \"image_generation\" } ] ```
    #[serde(rename = "tools")]
    pub tools: Vec<std::collections::HashMap<String, serde_json::Value>>,
}

impl ToolChoiceAllowed {
    /// Constrains the tools available to the model to a pre-defined set.
    pub fn new(
        r#type: ToolChoiceAllowedType,
        mode: ToolChoiceAllowedMode,
        tools: Vec<std::collections::HashMap<String, serde_json::Value>>,
    ) -> ToolChoiceAllowed {
        ToolChoiceAllowed { r#type, mode, tools }
    }
}
/// Allowed tool configuration type. Always `allowed_tools`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceAllowedType {
    #[serde(rename = "allowed_tools")]
    #[default]
    AllowedTools,
}

/// Constrains the tools available to the model to a pre-defined set.  `auto` allows the model to pick from among the allowed tools and generate a message.  `required` requires the model to call one or more of the allowed tools.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceAllowedMode {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "required")]
    Required,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceCustom {
    /// For custom tool calling, the type is always `custom`.
    #[serde(rename = "type")]
    pub r#type: ToolChoiceCustomType,
    /// The name of the custom tool to call.
    #[serde(rename = "name")]
    pub name: String,
}

impl ToolChoiceCustom {
    /// Use this option to force the model to call a specific custom tool.
    pub fn new(r#type: ToolChoiceCustomType, name: String) -> ToolChoiceCustom {
        ToolChoiceCustom { r#type, name }
    }
}
/// For custom tool calling, the type is always `custom`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceCustomType {
    #[serde(rename = "custom")]
    #[default]
    Custom,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// For function calling, the type is always `function`.
    #[serde(rename = "type")]
    pub r#type: ToolChoiceFunctionType,
    /// The name of the function to call.
    #[serde(rename = "name")]
    pub name: String,
}

impl ToolChoiceFunction {
    /// Use this option to force the model to call a specific function.
    pub fn new(r#type: ToolChoiceFunctionType, name: String) -> ToolChoiceFunction {
        ToolChoiceFunction { r#type, name }
    }
}
/// For function calling, the type is always `function`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceFunctionType {
    #[serde(rename = "function")]
    #[default]
    Function,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceMcp {
    /// For MCP tools, the type is always `mcp`.
    #[serde(rename = "type")]
    pub r#type: ToolChoiceMcpType,
    /// The label of the MCP server to use.
    #[serde(rename = "server_label")]
    pub server_label: String,
    /// The name of the tool to call on the server.
    #[serde(
        rename = "name",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<Option<String>>,
}

impl ToolChoiceMcp {
    /// Use this option to force the model to call a specific tool on a remote MCP server.
    pub fn new(r#type: ToolChoiceMcpType, server_label: String) -> ToolChoiceMcp {
        ToolChoiceMcp { r#type, server_label, name: None }
    }
}
/// For MCP tools, the type is always `mcp`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceMcpType {
    #[serde(rename = "mcp")]
    #[default]
    Mcp,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ToolChoiceOptions {
    #[serde(rename = "none")]
    #[default]
    None,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "required")]
    Required,
}

impl std::fmt::Display for ToolChoiceOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Auto => write!(f, "auto"),
            Self::Required => write!(f, "required"),
        }
    }
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoiceParam {
    ToolChoiceOptions(models::ToolChoiceOptions),
    ToolChoiceAllowed(Box<models::ToolChoiceAllowed>),
    ToolChoiceTypes(Box<models::ToolChoiceTypes>),
    ToolChoiceFunction(Box<models::ToolChoiceFunction>),
    ToolChoiceMcp(Box<models::ToolChoiceMcp>),
    ToolChoiceCustom(Box<models::ToolChoiceCustom>),
    SpecificApplyPatchParam(Box<models::SpecificApplyPatchParam>),
    SpecificFunctionShellParam(Box<models::SpecificFunctionShellParam>),
}

impl Default for ToolChoiceParam {
    fn default() -> Self {
        Self::ToolChoiceOptions(Default::default())
    }
}
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolChoiceTypes {
    /// The type of hosted tool the model should to use. Learn more about [built-in tools](/docs/guides/tools).  Allowed values are: - `file_search` - `web_search_preview` - `computer` - `computer_use_preview` - `computer_use` - `code_interpreter` - `image_generation`
    #[serde(rename = "type")]
    pub r#type: ToolChoiceTypesType,
}

impl ToolChoiceTypes {
    /// Indicates that the model should use a built-in tool to generate a response. [Learn more about built-in tools](/docs/guides/tools).
    pub fn new(r#type: ToolChoiceTypesType) -> ToolChoiceTypes {
        ToolChoiceTypes { r#type }
    }
}
/// The type of hosted tool the model should to use. Learn more about [built-in tools](/docs/guides/tools).  Allowed values are: - `file_search` - `web_search_preview` - `computer` - `computer_use_preview` - `computer_use` - `code_interpreter` - `image_generation`
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ToolChoiceTypesType {
    #[serde(rename = "file_search")]
    #[default]
    FileSearch,
    #[serde(rename = "web_search_preview")]
    WebSearchPreview,
    #[serde(rename = "computer")]
    Computer,
    #[serde(rename = "computer_use_preview")]
    ComputerUsePreview,
    #[serde(rename = "computer_use")]
    ComputerUse,
    #[serde(rename = "web_search_preview_2025_03_11")]
    WebSearchPreview20250311,
    #[serde(rename = "image_generation")]
    ImageGeneration,
    #[serde(rename = "code_interpreter")]
    CodeInterpreter,
}

