//! Placeholder OpenAI wire types referenced by generated schemas.
//!
//! These structs intentionally stay permissive until the upstream schema
//! exposes a more precise contract for each referenced shape.

use serde::{Deserialize, Serialize};

/// Token usage totals returned by completion-style responses.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletionUsage {
    #[serde(rename = "prompt_tokens", skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<i32>,
    #[serde(rename = "completion_tokens", skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<i32>,
    #[serde(rename = "total_tokens", skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<i32>,
}

/// Output resource returned from computer tool calls.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerToolCallOutputResource {
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Parameter object for submitting computer-call output items.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerCallOutputItemParam {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Computer tool declaration placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerTool {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Computer-use preview tool declaration placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComputerUsePreviewTool {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Code interpreter tool declaration placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CodeInterpreterTool {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// File search tool declaration placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileSearchTool {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Citation metadata for files produced by a hosted container.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContainerFileCitationBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

/// Generic named item field used by generated response schemas.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemField {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Text message content object used by request schemas.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageRequestContentTextObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// File-detail discriminator placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileDetailEnum {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// File input detail placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileInputDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// File-path payload placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FilePath {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// File annotation placeholder used by generated response content.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// File citation body placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileCitationBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
}

/// Template message item placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputMessagesTemplateTemplateInner {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Alias for equivalent template item shapes generated under different names.
pub type TemplateInputMessagesTemplateInner = InputMessagesTemplateTemplateInner;

/// Chatkit workflow reference placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatkitWorkflow {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Chatkit configuration reference placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatkitConfigurationParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Developer-message placeholder for chat completion requests.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestDeveloperMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Function-call payload placeholder on assistant messages.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequestAssistantMessageFunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Named function tool choice placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssistantsNamedToolChoiceFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Image-file message content placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageContentImageFileObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Image-URL message content placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageContentImageUrlObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

/// Compact response service tier subset. Keep separate from `ServiceTier`
/// because this generated shape only accepts `auto` and `default`.
#[derive(
    Clone, Copy, Default, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize,
)]
pub enum ServiceTierEnum {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "default")]
    Default,
}

/// Conversation reference placeholder for generated response shapes.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conversation2 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Shell-call environment placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionShellCallEnvironment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Shell-call item parameter environment placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionShellCallItemParamEnvironment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Shell-tool parameter environment placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionShellToolParamEnvironment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Input token-count request placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenCountsBodyInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<i32>,
}

/// Approximate geographic location placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApproximateLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}

/// Chat session chatkit configuration placeholder.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatSessionChatkitConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
