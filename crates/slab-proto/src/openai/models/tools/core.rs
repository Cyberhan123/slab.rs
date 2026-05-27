
use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Tool {
    #[serde(rename = "FunctionTool")]
    FunctionTool(Box<models::FunctionTool>),
    #[serde(rename = "FileSearchTool")]
    FileSearchTool(Box<models::FileSearchTool>),
    #[serde(rename = "ComputerTool")]
    ComputerTool(Box<models::ComputerTool>),
    #[serde(rename = "ComputerUsePreviewTool")]
    ComputerUsePreviewTool(Box<models::ComputerUsePreviewTool>),
    #[serde(rename = "WebSearchTool")]
    WebSearchTool(Box<models::WebSearchTool>),
    #[serde(rename = "MCPTool")]
    McpTool(Box<models::McpTool>),
    #[serde(rename = "CodeInterpreterTool")]
    CodeInterpreterTool(Box<models::CodeInterpreterTool>),
    #[serde(rename = "ImageGenTool")]
    ImageGenTool(Box<models::ImageGenTool>),
    #[serde(rename = "LocalShellToolParam")]
    LocalShellToolParam(Box<models::LocalShellToolParam>),
    #[serde(rename = "FunctionShellToolParam")]
    FunctionShellToolParam(Box<models::FunctionShellToolParam>),
    #[serde(rename = "CustomToolParam")]
    CustomToolParam(Box<models::CustomToolParam>),
    #[serde(rename = "NamespaceToolParam")]
    NamespaceToolParam(Box<models::NamespaceToolParam>),
    #[serde(rename = "ToolSearchToolParam")]
    ToolSearchToolParam(Box<models::ToolSearchToolParam>),
    #[serde(rename = "WebSearchPreviewTool")]
    WebSearchPreviewTool(Box<models::WebSearchPreviewTool>),
    #[serde(rename = "ApplyPatchToolParam")]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum SubmitToolOutputsRunRequestToolOutputsInnerConnectorId {
    #[serde(rename = "connector_dropbox")]
    ConnectorDropbox,
    #[serde(rename = "connector_gmail")]
    ConnectorGmail,
    #[serde(rename = "connector_googlecalendar")]
    ConnectorGooglecalendar,
    #[serde(rename = "connector_googledrive")]
    ConnectorGoogledrive,
    #[serde(rename = "connector_microsoftteams")]
    ConnectorMicrosoftteams,
    #[serde(rename = "connector_outlookcalendar")]
    ConnectorOutlookcalendar,
    #[serde(rename = "connector_outlookemail")]
    ConnectorOutlookemail,
    #[serde(rename = "connector_sharepoint")]
    ConnectorSharepoint,
}

impl Default for SubmitToolOutputsRunRequestToolOutputsInnerConnectorId {
    fn default() -> SubmitToolOutputsRunRequestToolOutputsInnerConnectorId {
        Self::ConnectorDropbox
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamespaceToolParam {
    /// The type of the tool. Always `namespace`.
    #[serde(rename = "type")]
    pub r#type: Type,
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
        r#type: Type,
        name: String,
        description: String,
        tools: Vec<models::NamespaceToolParamToolsInner>,
    ) -> NamespaceToolParam {
        NamespaceToolParam { r#type, name, description, tools }
    }
}
/// The type of the tool. Always `namespace`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub(crate) enum Type {
    #[serde(rename = "namespace")]
    Namespace,
}

impl Default for Type {
    fn default() -> Type {
        Self::Namespace
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NamespaceToolParamToolsInner {
    #[serde(rename = "FunctionToolParam")]
    FunctionToolParam(Box<models::FunctionToolParam>),
    #[serde(rename = "CustomToolParam")]
    CustomToolParam(Box<models::CustomToolParam>),
}

impl Default for NamespaceToolParamToolsInner {
    fn default() -> Self {
        Self::FunctionToolParam(Default::default())
    }
}
