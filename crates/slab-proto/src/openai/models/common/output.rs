use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputContent {
    #[serde(rename = "OutputTextContent")]
    OutputTextContent(Box<models::OutputTextContent>),
    #[serde(rename = "RefusalContent")]
    RefusalContent(Box<models::RefusalContent>),
    #[serde(rename = "ReasoningTextContent")]
    ReasoningTextContent(Box<models::ReasoningTextContent>),
}

impl Default for OutputContent {
    fn default() -> Self {
        Self::OutputTextContent(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputMessageContent {
    #[serde(rename = "OutputTextContent")]
    OutputTextContent(Box<models::OutputTextContent>),
    #[serde(rename = "RefusalContent")]
    RefusalContent(Box<models::RefusalContent>),
}

impl Default for OutputMessageContent {
    fn default() -> Self {
        Self::OutputTextContent(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "OutputMessage")]
    OutputMessage(Box<models::OutputMessage>),
    #[serde(rename = "FileSearchToolCall")]
    FileSearchToolCall(Box<models::FileSearchToolCall>),
    #[serde(rename = "FunctionToolCall")]
    FunctionToolCall(Box<models::FunctionToolCall>),
    #[serde(rename = "FunctionToolCallOutputResource")]
    FunctionToolCallOutputResource(Box<models::FunctionToolCallOutputResource>),
    #[serde(rename = "WebSearchToolCall")]
    WebSearchToolCall(Box<models::WebSearchToolCall>),
    #[serde(rename = "ComputerToolCall")]
    ComputerToolCall(Box<models::ComputerToolCall>),
    #[serde(rename = "ComputerToolCallOutputResource")]
    ComputerToolCallOutputResource(Box<models::ComputerToolCallOutputResource>),
    #[serde(rename = "ReasoningItem")]
    ReasoningItem(Box<models::ReasoningItem>),
    #[serde(rename = "ToolSearchCall")]
    ToolSearchCall(Box<models::ToolSearchCall>),
    #[serde(rename = "ToolSearchOutput")]
    ToolSearchOutput(Box<models::ToolSearchOutput>),
    #[serde(rename = "CompactionBody")]
    CompactionBody(Box<models::CompactionBody>),
    #[serde(rename = "ImageGenToolCall")]
    ImageGenToolCall(Box<models::ImageGenToolCall>),
    #[serde(rename = "CodeInterpreterToolCall")]
    CodeInterpreterToolCall(Box<models::CodeInterpreterToolCall>),
    #[serde(rename = "LocalShellToolCall")]
    LocalShellToolCall(Box<models::LocalShellToolCall>),
    #[serde(rename = "LocalShellToolCallOutput")]
    LocalShellToolCallOutput(Box<models::LocalShellToolCallOutput>),
    #[serde(rename = "FunctionShellCall")]
    FunctionShellCall(Box<models::FunctionShellCall>),
    #[serde(rename = "FunctionShellCallOutput")]
    FunctionShellCallOutput(Box<models::FunctionShellCallOutput>),
    #[serde(rename = "ApplyPatchToolCall")]
    ApplyPatchToolCall(Box<models::ApplyPatchToolCall>),
    #[serde(rename = "ApplyPatchToolCallOutput")]
    ApplyPatchToolCallOutput(Box<models::ApplyPatchToolCallOutput>),
    #[serde(rename = "MCPToolCall")]
    McpToolCall(Box<models::McpToolCall>),
    #[serde(rename = "MCPListTools")]
    McpListTools(Box<models::McpListTools>),
    #[serde(rename = "MCPApprovalRequest")]
    McpApprovalRequest(Box<models::McpApprovalRequest>),
    #[serde(rename = "MCPApprovalResponseResource")]
    McpApprovalResponseResource(Box<models::McpApprovalResponseResource>),
    #[serde(rename = "CustomToolCall")]
    CustomToolCall(Box<models::CustomToolCall>),
    #[serde(rename = "CustomToolCallOutputResource")]
    CustomToolCallOutputResource(Box<models::CustomToolCallOutputResource>),
}

impl Default for OutputItem {
    fn default() -> Self {
        Self::OutputMessage(Default::default())
    }
}

/// The role of the output message. Always `assistant`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum OutputItemRole {
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}


use super::status::Status;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputMessage {
    /// The unique ID of the output message.
    #[serde(rename = "id")]
    pub id: String,
    /// The type of the output message. Always `message`.
    #[serde(rename = "type")]
    pub r#type: Type,
    /// The role of the output message. Always `assistant`.
    #[serde(rename = "role")]
    pub role: OutputMessageRole,
    /// The content of the output message.
    #[serde(rename = "content")]
    pub content: Vec<models::OutputMessageContent>,
    /// The status of the message input. One of `in_progress`, `completed`, or `incomplete`. Populated when input items are returned via API.
    #[serde(rename = "status")]
    pub status: Status,
    #[serde(
        rename = "phase",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub phase: Option<Option<models::MessagePhase>>,
}

impl OutputMessage {
    /// An output message from the model.
    pub fn new(
        id: String,
        r#type: Type,
        role: OutputMessageRole,
        content: Vec<models::OutputMessageContent>,
        status: Status,
    ) -> OutputMessage {
        OutputMessage { id, r#type, role, content, status, phase: None }
    }
}
/// The type of the output message. Always `message`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum Type {
    #[serde(rename = "message")]
    #[default]
    Message,
}

/// The role of the output message. Always `assistant`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum OutputMessageRole {
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ItemResource {
    #[serde(rename = "InputMessageResource")]
    InputMessageResource(Box<models::InputMessageResource>),
    #[serde(rename = "OutputMessage")]
    OutputMessage(Box<models::OutputMessage>),
    #[serde(rename = "FileSearchToolCall")]
    FileSearchToolCall(Box<models::FileSearchToolCall>),
    #[serde(rename = "ComputerToolCall")]
    ComputerToolCall(Box<models::ComputerToolCall>),
    #[serde(rename = "ComputerToolCallOutputResource")]
    ComputerToolCallOutputResource(Box<models::ComputerToolCallOutputResource>),
    #[serde(rename = "WebSearchToolCall")]
    WebSearchToolCall(Box<models::WebSearchToolCall>),
    #[serde(rename = "FunctionToolCallResource")]
    FunctionToolCallResource(Box<models::FunctionToolCallResource>),
    #[serde(rename = "FunctionToolCallOutputResource")]
    FunctionToolCallOutputResource(Box<models::FunctionToolCallOutputResource>),
    #[serde(rename = "ToolSearchCall")]
    ToolSearchCall(Box<models::ToolSearchCall>),
    #[serde(rename = "ToolSearchOutput")]
    ToolSearchOutput(Box<models::ToolSearchOutput>),
    #[serde(rename = "ReasoningItem")]
    ReasoningItem(Box<models::ReasoningItem>),
    #[serde(rename = "CompactionBody")]
    CompactionBody(Box<models::CompactionBody>),
    #[serde(rename = "ImageGenToolCall")]
    ImageGenToolCall(Box<models::ImageGenToolCall>),
    #[serde(rename = "CodeInterpreterToolCall")]
    CodeInterpreterToolCall(Box<models::CodeInterpreterToolCall>),
    #[serde(rename = "LocalShellToolCall")]
    LocalShellToolCall(Box<models::LocalShellToolCall>),
    #[serde(rename = "LocalShellToolCallOutput")]
    LocalShellToolCallOutput(Box<models::LocalShellToolCallOutput>),
    #[serde(rename = "FunctionShellCall")]
    FunctionShellCall(Box<models::FunctionShellCall>),
    #[serde(rename = "FunctionShellCallOutput")]
    FunctionShellCallOutput(Box<models::FunctionShellCallOutput>),
    #[serde(rename = "ApplyPatchToolCall")]
    ApplyPatchToolCall(Box<models::ApplyPatchToolCall>),
    #[serde(rename = "ApplyPatchToolCallOutput")]
    ApplyPatchToolCallOutput(Box<models::ApplyPatchToolCallOutput>),
    #[serde(rename = "MCPListTools")]
    McpListTools(Box<models::McpListTools>),
    #[serde(rename = "MCPApprovalRequest")]
    McpApprovalRequest(Box<models::McpApprovalRequest>),
    #[serde(rename = "MCPApprovalResponseResource")]
    McpApprovalResponseResource(Box<models::McpApprovalResponseResource>),
    #[serde(rename = "MCPToolCall")]
    McpToolCall(Box<models::McpToolCall>),
    #[serde(rename = "CustomToolCallResource")]
    CustomToolCallResource(Box<models::CustomToolCallResource>),
    #[serde(rename = "CustomToolCallOutputResource")]
    CustomToolCallOutputResource(Box<models::CustomToolCallOutputResource>),
}

impl Default for ItemResource {
    fn default() -> Self {
        Self::InputMessageResource(Default::default())
    }
}

/// The role of the output message. Always `assistant`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub(crate) enum ItemResourceRole {
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}

