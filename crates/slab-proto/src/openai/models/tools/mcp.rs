use crate::openai::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpApprovalRequest {
    /// The type of the item. Always `mcp_approval_request`.
    #[serde(rename = "type")]
    pub r#type: McpApprovalRequestType,
    /// The unique ID of the approval request.
    #[serde(rename = "id")]
    pub id: String,
    /// The label of the MCP server making the request.
    #[serde(rename = "server_label")]
    pub server_label: String,
    /// The name of the tool to run.
    #[serde(rename = "name")]
    pub name: String,
    /// A JSON string of arguments for the tool.
    #[serde(rename = "arguments")]
    pub arguments: String,
}

impl McpApprovalRequest {
    /// A request for human approval of a tool invocation.
    pub fn new(
        r#type: McpApprovalRequestType,
        id: String,
        server_label: String,
        name: String,
        arguments: String,
    ) -> McpApprovalRequest {
        McpApprovalRequest { r#type, id, server_label, name, arguments }
    }
}
/// The type of the item. Always `mcp_approval_request`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpApprovalRequestType {
    #[serde(rename = "mcp_approval_request")]
    #[default]
    McpApprovalRequest,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpApprovalResponse {
    /// The type of the item. Always `mcp_approval_response`.
    #[serde(rename = "type")]
    pub r#type: McpApprovalResponseType,
    /// The ID of the approval request being answered.
    #[serde(rename = "approval_request_id")]
    pub approval_request_id: String,
    /// Whether the request was approved.
    #[serde(rename = "approve")]
    pub approve: bool,
    /// The unique ID of the approval response
    #[serde(
        rename = "id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub id: Option<Option<String>>,
    /// Optional reason for the decision.
    #[serde(
        rename = "reason",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub reason: Option<Option<String>>,
}

impl McpApprovalResponse {
    /// A response to an MCP approval request.
    pub fn new(
        r#type: McpApprovalResponseType,
        approval_request_id: String,
        approve: bool,
    ) -> McpApprovalResponse {
        McpApprovalResponse { r#type, approval_request_id, approve, id: None, reason: None }
    }
}
/// The type of the item. Always `mcp_approval_response`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpApprovalResponseType {
    #[serde(rename = "mcp_approval_response")]
    #[default]
    McpApprovalResponse,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpApprovalResponseResource {
    /// The type of the item. Always `mcp_approval_response`.
    #[serde(rename = "type")]
    pub r#type: McpApprovalResponseResourceType,
    /// The unique ID of the approval response
    #[serde(rename = "id")]
    pub id: String,
    /// The ID of the approval request being answered.
    #[serde(rename = "approval_request_id")]
    pub approval_request_id: String,
    /// Whether the request was approved.
    #[serde(rename = "approve")]
    pub approve: bool,
    /// Optional reason for the decision.
    #[serde(
        rename = "reason",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub reason: Option<Option<String>>,
}

impl McpApprovalResponseResource {
    /// A response to an MCP approval request.
    pub fn new(
        r#type: McpApprovalResponseResourceType,
        id: String,
        approval_request_id: String,
        approve: bool,
    ) -> McpApprovalResponseResource {
        McpApprovalResponseResource { r#type, id, approval_request_id, approve, reason: None }
    }
}
/// The type of the item. Always `mcp_approval_response`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpApprovalResponseResourceType {
    #[serde(rename = "mcp_approval_response")]
    #[default]
    McpApprovalResponse,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpListTools {
    /// The type of the item. Always `mcp_list_tools`.
    #[serde(rename = "type")]
    pub r#type: McpListToolsType,
    /// The unique ID of the list.
    #[serde(rename = "id")]
    pub id: String,
    /// The label of the MCP server.
    #[serde(rename = "server_label")]
    pub server_label: String,
    /// The tools available on the server.
    #[serde(rename = "tools")]
    pub tools: Vec<models::McpListToolsTool>,
    /// Error message if the server could not list tools.
    #[serde(
        rename = "error",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub error: Option<Option<String>>,
}

impl McpListTools {
    /// A list of tools available on an MCP server.
    pub fn new(
        r#type: McpListToolsType,
        id: String,
        server_label: String,
        tools: Vec<models::McpListToolsTool>,
    ) -> McpListTools {
        McpListTools { r#type, id, server_label, tools, error: None }
    }
}
/// The type of the item. Always `mcp_list_tools`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpListToolsType {
    #[serde(rename = "mcp_list_tools")]
    #[default]
    McpListTools,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpListToolsTool {
    /// The name of the tool.
    #[serde(rename = "name")]
    pub name: String,
    /// The JSON schema describing the tool's input.
    #[serde(rename = "input_schema")]
    pub input_schema: serde_json::Value,
    /// The description of the tool.
    #[serde(
        rename = "description",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub description: Option<Option<String>>,
    /// Additional annotations about the tool.
    #[serde(
        rename = "annotations",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub annotations: Option<Option<serde_json::Value>>,
}

impl McpListToolsTool {
    /// A tool available on an MCP server.
    pub fn new(name: String, input_schema: serde_json::Value) -> McpListToolsTool {
        McpListToolsTool { name, input_schema, description: None, annotations: None }
    }
}

use super::core::SubmitToolOutputsRunRequestToolOutputsInnerConnectorId as ConnectorId;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpTool {
    /// The type of the MCP tool. Always `mcp`.
    #[serde(rename = "type")]
    pub r#type: McpToolType,
    /// A label for this MCP server, used to identify it in tool calls.
    #[serde(rename = "server_label")]
    pub server_label: String,
    /// The URL for the MCP server. One of `server_url` or `connector_id` must be provided.
    #[serde(rename = "server_url", skip_serializing_if = "Option::is_none")]
    pub server_url: Option<String>,
    /// Identifier for service connectors, like those available in ChatGPT. One of `server_url` or `connector_id` must be provided. Learn more about service connectors [here](/docs/guides/tools-remote-mcp#connectors).  Currently supported `connector_id` values are:  - Dropbox: `connector_dropbox` - Gmail: `connector_gmail` - Google Calendar: `connector_googlecalendar` - Google Drive: `connector_googledrive` - Microsoft Teams: `connector_microsoftteams` - Outlook Calendar: `connector_outlookcalendar` - Outlook Email: `connector_outlookemail` - SharePoint: `connector_sharepoint`
    #[serde(rename = "connector_id", skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<ConnectorId>,
    /// An OAuth access token that can be used with a remote MCP server, either with a custom MCP server URL or a service connector. Your application must handle the OAuth authorization flow and provide the token here.
    #[serde(rename = "authorization", skip_serializing_if = "Option::is_none")]
    pub authorization: Option<String>,
    /// Optional description of the MCP server, used to provide more context.
    #[serde(rename = "server_description", skip_serializing_if = "Option::is_none")]
    pub server_description: Option<String>,
    /// Optional HTTP headers to send to the MCP server. Use for authentication or other purposes.
    #[serde(
        rename = "headers",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub headers: Option<Option<std::collections::HashMap<String, String>>>,
    #[serde(
        rename = "allowed_tools",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub allowed_tools: Option<Option<Box<models::McpToolAllowedTools>>>,
    #[serde(
        rename = "require_approval",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub require_approval: Option<Option<Box<models::McpToolRequireApproval>>>,
    /// Whether this MCP tool is deferred and discovered via tool search.
    #[serde(rename = "defer_loading", skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
}

impl McpTool {
    /// Give the model access to additional tools via remote Model Context Protocol (MCP) servers. [Learn more about MCP](/docs/guides/tools-remote-mcp).
    pub fn new(r#type: McpToolType, server_label: String) -> McpTool {
        McpTool {
            r#type,
            server_label,
            server_url: None,
            connector_id: None,
            authorization: None,
            server_description: None,
            headers: None,
            allowed_tools: None,
            require_approval: None,
            defer_loading: None,
        }
    }
}
/// The type of the MCP tool. Always `mcp`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpToolType {
    #[serde(rename = "mcp")]
    #[default]
    Mcp,
}

// Identifier for service connectors, like those available in ChatGPT. One of `server_url` or `connector_id` must be provided. Learn more about service connectors [here](/docs/guides/tools-remote-mcp#connectors).  Currently supported `connector_id` values are:  - Dropbox: `connector_dropbox` - Gmail: `connector_gmail` - Google Calendar: `connector_googlecalendar` - Google Drive: `connector_googledrive` - Microsoft Teams: `connector_microsoftteams` - Outlook Calendar: `connector_outlookcalendar` - Outlook Email: `connector_outlookemail` - SharePoint: `connector_sharepoint`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpToolAllowedTools {
    /// A string array of allowed tool names
    McpAllowedTools(Vec<String>),
    McpToolFilter(Box<models::McpToolFilter>),
}

impl Default for McpToolAllowedTools {
    fn default() -> Self {
        Self::McpAllowedTools(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpToolApprovalFilter {
    #[serde(rename = "always", skip_serializing_if = "Option::is_none")]
    pub always: Option<Box<models::McpToolFilter>>,
    #[serde(rename = "never", skip_serializing_if = "Option::is_none")]
    pub never: Option<Box<models::McpToolFilter>>,
}

impl McpToolApprovalFilter {
    /// Specify which of the MCP server's tools require approval. Can be `always`, `never`, or a filter object associated with tools that require approval.
    pub fn new() -> McpToolApprovalFilter {
        McpToolApprovalFilter { always: None, never: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpToolCall {
    /// The type of the item. Always `mcp_call`.
    #[serde(rename = "type")]
    pub r#type: McpToolCallType,
    /// The unique ID of the tool call.
    #[serde(rename = "id")]
    pub id: String,
    /// The label of the MCP server running the tool.
    #[serde(rename = "server_label")]
    pub server_label: String,
    /// The name of the tool that was run.
    #[serde(rename = "name")]
    pub name: String,
    /// A JSON string of the arguments passed to the tool.
    #[serde(rename = "arguments")]
    pub arguments: String,
    /// The output from the tool call.
    #[serde(
        rename = "output",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub output: Option<Option<String>>,
    /// The error from the tool call, if any.
    #[serde(
        rename = "error",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub error: Option<Option<String>>,
    /// The status of the tool call. One of `in_progress`, `completed`, `incomplete`, `calling`, or `failed`.
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<models::McpToolCallStatus>,
    /// Unique identifier for the MCP tool call approval request. Include this value in a subsequent `mcp_approval_response` input to approve or reject the corresponding tool call.
    #[serde(
        rename = "approval_request_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub approval_request_id: Option<Option<String>>,
}

impl McpToolCall {
    /// An invocation of a tool on an MCP server.
    pub fn new(
        r#type: McpToolCallType,
        id: String,
        server_label: String,
        name: String,
        arguments: String,
    ) -> McpToolCall {
        McpToolCall {
            r#type,
            id,
            server_label,
            name,
            arguments,
            output: None,
            error: None,
            status: None,
            approval_request_id: None,
        }
    }
}
/// The type of the item. Always `mcp_call`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpToolCallType {
    #[serde(rename = "mcp_call")]
    #[default]
    McpCall,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum McpToolCallStatus {
    #[serde(rename = "in_progress")]
    #[default]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
    #[serde(rename = "calling")]
    Calling,
    #[serde(rename = "failed")]
    Failed,
}

impl std::fmt::Display for McpToolCallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Incomplete => write!(f, "incomplete"),
            Self::Calling => write!(f, "calling"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpToolFilter {
    /// List of allowed tool names.
    #[serde(rename = "tool_names", skip_serializing_if = "Option::is_none")]
    pub tool_names: Option<Vec<String>>,
    /// Indicates whether or not a tool modifies data or is read-only. If an MCP server is [annotated with `readOnlyHint`](https://modelcontextprotocol.io/specification/2025-06-18/schema#toolannotations-readonlyhint), it will match this filter.
    #[serde(rename = "read_only", skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
}

impl McpToolFilter {
    /// A filter object to specify which tools are allowed.
    pub fn new() -> McpToolFilter {
        McpToolFilter { tool_names: None, read_only: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpToolRequireApproval {
    McpToolApprovalFilter(Box<models::McpToolApprovalFilter>),
    /// Specify a single approval policy for all tools. One of `always` or `never`. When set to `always`, all tools will require approval. When set to `never`, all tools will not require approval.
    McpToolApprovalSetting(String),
}

impl Default for McpToolRequireApproval {
    fn default() -> Self {
        Self::McpToolApprovalFilter(Default::default())
    }
}
