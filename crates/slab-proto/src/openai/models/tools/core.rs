use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Tool {
    #[serde(rename = "function")]
    FunctionTool(Box<models::FunctionTool>),
    #[serde(rename = "file_search")]
    FileSearchTool(Box<models::FileSearchTool>),
    #[serde(rename = "computer")]
    ComputerTool(Box<models::ComputerTool>),
    #[serde(rename = "computer_use_preview")]
    ComputerUsePreviewTool(Box<models::ComputerUsePreviewTool>),
    #[serde(rename = "web_search")]
    WebSearchTool(Box<models::WebSearchTool>),
    #[serde(rename = "mcp")]
    McpTool(Box<models::McpTool>),
    #[serde(rename = "code_interpreter")]
    CodeInterpreterTool(Box<models::CodeInterpreterTool>),
    #[serde(rename = "image_generation")]
    ImageGenTool(Box<models::ImageGenTool>),
    #[serde(rename = "local_shell")]
    LocalShellToolParam(Box<models::LocalShellToolParam>),
    #[serde(rename = "shell")]
    FunctionShellToolParam(Box<models::FunctionShellToolParam>),
    #[serde(rename = "custom")]
    CustomToolParam(Box<models::CustomToolParam>),
    #[serde(rename = "namespace")]
    NamespaceToolParam(Box<models::NamespaceToolParam>),
    #[serde(rename = "tool_search")]
    ToolSearchToolParam(Box<models::ToolSearchToolParam>),
    #[serde(rename = "web_search_preview")]
    WebSearchPreviewTool(Box<models::WebSearchPreviewTool>),
    #[serde(rename = "apply_patch")]
    ApplyPatchToolParam(Box<models::ApplyPatchToolParam>),
}

impl Default for Tool {
    fn default() -> Self {
        Self::FunctionTool(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitToolOutputsRunRequestToolOutputsInner {
    /// The ID of the tool call in the `required_action` object within the run object the output is being submitted for.
    #[serde(rename = "tool_call_id", skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// The output of the tool call to be submitted to continue the run.
    #[serde(rename = "output", skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl SubmitToolOutputsRunRequestToolOutputsInner {
    pub fn new() -> SubmitToolOutputsRunRequestToolOutputsInner {
        SubmitToolOutputsRunRequestToolOutputsInner { tool_call_id: None, output: None }
    }
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum SubmitToolOutputsRunRequestToolOutputsInnerConnectorId {
    #[serde(rename = "connector_dropbox")]
    #[default]
    Dropbox,
    #[serde(rename = "connector_gmail")]
    Gmail,
    #[serde(rename = "connector_googlecalendar")]
    Googlecalendar,
    #[serde(rename = "connector_googledrive")]
    Googledrive,
    #[serde(rename = "connector_microsoftteams")]
    Microsoftteams,
    #[serde(rename = "connector_outlookcalendar")]
    Outlookcalendar,
    #[serde(rename = "connector_outlookemail")]
    Outlookemail,
    #[serde(rename = "connector_sharepoint")]
    Sharepoint,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamespaceToolParam {
    /// The namespace name used in tool calls (for example, `crm`).
    #[serde(rename = "name")]
    pub name: String,
    /// A description of the namespace shown to the model.
    #[serde(rename = "description")]
    pub description: String,
    /// The function/custom tools available inside this namespace.
    #[serde(rename = "tools")]
    pub tools: Vec<models::NamespaceToolParamToolsInner>,
}

impl NamespaceToolParam {
    /// Groups function/custom tools under a shared namespace.
    pub fn new(
        name: String,
        description: String,
        tools: Vec<models::NamespaceToolParamToolsInner>,
    ) -> NamespaceToolParam {
        NamespaceToolParam { name, description, tools }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NamespaceToolParamToolsInner {
    #[serde(rename = "function")]
    FunctionToolParam(Box<models::FunctionToolParam>),
    #[serde(rename = "custom")]
    CustomToolParam(Box<models::CustomToolParam>),
}

impl Default for NamespaceToolParamToolsInner {
    fn default() -> Self {
        Self::FunctionToolParam(Default::default())
    }
}
