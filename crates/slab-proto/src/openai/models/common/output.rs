use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputContent {
    OutputTextContent(Box<models::OutputTextContent>),
    RefusalContent(Box<models::RefusalContent>),
    ReasoningTextContent(Box<models::ReasoningTextContent>),
}

impl Default for OutputContent {
    fn default() -> Self {
        Self::OutputTextContent(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputMessageContent {
    OutputTextContent(Box<models::OutputTextContent>),
    RefusalContent(Box<models::RefusalContent>),
}

impl Default for OutputMessageContent {
    fn default() -> Self {
        Self::OutputTextContent(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputItem {
    OutputMessage(Box<models::OutputMessage>),
    FileSearchToolCall(Box<models::FileSearchToolCall>),
    FunctionToolCall(Box<models::FunctionToolCall>),
    FunctionToolCallOutputResource(Box<models::FunctionToolCallOutputResource>),
    WebSearchToolCall(Box<models::WebSearchToolCall>),
    ComputerToolCall(Box<models::ComputerToolCall>),
    ComputerToolCallOutputResource(Box<models::ComputerToolCallOutputResource>),
    ReasoningItem(Box<models::ReasoningItem>),
    ToolSearchCall(Box<models::ToolSearchCall>),
    ToolSearchOutput(Box<models::ToolSearchOutput>),
    CompactionBody(Box<models::CompactionBody>),
    ImageGenToolCall(Box<models::ImageGenToolCall>),
    CodeInterpreterToolCall(Box<models::CodeInterpreterToolCall>),
    LocalShellToolCall(Box<models::LocalShellToolCall>),
    LocalShellToolCallOutput(Box<models::LocalShellToolCallOutput>),
    FunctionShellCall(Box<models::FunctionShellCall>),
    FunctionShellCallOutput(Box<models::FunctionShellCallOutput>),
    ApplyPatchToolCall(Box<models::ApplyPatchToolCall>),
    ApplyPatchToolCallOutput(Box<models::ApplyPatchToolCallOutput>),
    McpToolCall(Box<models::McpToolCall>),
    McpListTools(Box<models::McpListTools>),
    McpApprovalRequest(Box<models::McpApprovalRequest>),
    McpApprovalResponseResource(Box<models::McpApprovalResponseResource>),
    CustomToolCall(Box<models::CustomToolCall>),
    CustomToolCallOutputResource(Box<models::CustomToolCallOutputResource>),
}

impl Default for OutputItem {
    fn default() -> Self {
        Self::OutputMessage(Default::default())
    }
}

use super::status::Status;
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputMessage {
    /// The unique ID of the output message.
    #[serde(rename = "id")]
    pub id: String,
    /// The type of the output message. Always `message`.
    #[serde(rename = "type")]
    pub r#type: CommonOutputType,
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
        r#type: CommonOutputType,
        role: OutputMessageRole,
        content: Vec<models::OutputMessageContent>,
        status: Status,
    ) -> OutputMessage {
        OutputMessage { id, r#type, role, content, status, phase: None }
    }
}
/// The type of the output message. Always `message`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum CommonOutputType {
    #[serde(rename = "message")]
    #[default]
    Message,
}

/// The role of the output message. Always `assistant`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum OutputMessageRole {
    #[serde(rename = "assistant")]
    #[default]
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemResource {
    InputMessageResource(Box<models::InputMessageResource>),
    OutputMessage(Box<models::OutputMessage>),
    FileSearchToolCall(Box<models::FileSearchToolCall>),
    ComputerToolCall(Box<models::ComputerToolCall>),
    ComputerToolCallOutputResource(Box<models::ComputerToolCallOutputResource>),
    WebSearchToolCall(Box<models::WebSearchToolCall>),
    FunctionToolCallResource(Box<models::FunctionToolCallResource>),
    FunctionToolCallOutputResource(Box<models::FunctionToolCallOutputResource>),
    ToolSearchCall(Box<models::ToolSearchCall>),
    ToolSearchOutput(Box<models::ToolSearchOutput>),
    ReasoningItem(Box<models::ReasoningItem>),
    CompactionBody(Box<models::CompactionBody>),
    ImageGenToolCall(Box<models::ImageGenToolCall>),
    CodeInterpreterToolCall(Box<models::CodeInterpreterToolCall>),
    LocalShellToolCall(Box<models::LocalShellToolCall>),
    LocalShellToolCallOutput(Box<models::LocalShellToolCallOutput>),
    FunctionShellCall(Box<models::FunctionShellCall>),
    FunctionShellCallOutput(Box<models::FunctionShellCallOutput>),
    ApplyPatchToolCall(Box<models::ApplyPatchToolCall>),
    ApplyPatchToolCallOutput(Box<models::ApplyPatchToolCallOutput>),
    McpListTools(Box<models::McpListTools>),
    McpApprovalRequest(Box<models::McpApprovalRequest>),
    McpApprovalResponseResource(Box<models::McpApprovalResponseResource>),
    McpToolCall(Box<models::McpToolCall>),
    CustomToolCallResource(Box<models::CustomToolCallResource>),
    CustomToolCallOutputResource(Box<models::CustomToolCallOutputResource>),
}

impl Default for ItemResource {
    fn default() -> Self {
        Self::InputMessageResource(Default::default())
    }
}
