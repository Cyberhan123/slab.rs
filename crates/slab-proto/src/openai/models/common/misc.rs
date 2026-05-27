use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Anchor {
    #[serde(rename = "created_at")]
    #[default]
    CreatedAt,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientToolCallItem {
    /// Identifier of the thread item.
    #[serde(rename = "id")]
    pub id: String,
    /// Type discriminator that is always `chatkit.thread_item`.
    #[serde(rename = "object")]
    pub object: ClientToolCallItemObject,
    /// Unix timestamp (in seconds) for when the item was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Identifier of the parent thread.
    #[serde(rename = "thread_id")]
    pub thread_id: String,
    /// Type discriminator that is always `chatkit.client_tool_call`.
    #[serde(rename = "type")]
    pub r#type: ClientToolCallItemType,
    /// Execution status for the tool call.
    #[serde(rename = "status")]
    pub status: models::ClientToolCallStatus,
    /// Identifier for the client tool call.
    #[serde(rename = "call_id")]
    pub call_id: String,
    /// Tool name that was invoked.
    #[serde(rename = "name")]
    pub name: String,
    /// JSON-encoded arguments that were sent to the tool.
    #[serde(rename = "arguments")]
    pub arguments: String,
    /// JSON-encoded output captured from the tool. Defaults to null while execution is in progress.
    #[serde(rename = "output", deserialize_with = "Option::deserialize")]
    pub output: Option<String>,
}

impl ClientToolCallItem {
    /// Record of a client side tool invocation initiated by the assistant.
    pub fn new(
        id: String,
        object: ClientToolCallItemObject,
        created_at: i32,
        thread_id: String,
        r#type: ClientToolCallItemType,
        status: models::ClientToolCallStatus,
        call_id: String,
        name: String,
        arguments: String,
        output: Option<String>,
    ) -> ClientToolCallItem {
        ClientToolCallItem {
            id,
            object,
            created_at,
            thread_id,
            r#type,
            status,
            call_id,
            name,
            arguments,
            output,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Error {
    #[serde(rename = "code", deserialize_with = "Option::deserialize")]
    pub code: Option<String>,
    #[serde(rename = "message")]
    pub message: String,
    #[serde(rename = "param", deserialize_with = "Option::deserialize")]
    pub param: Option<String>,
    #[serde(rename = "type")]
    pub r#type: String,
}

impl Error {
    pub fn new(
        code: Option<String>,
        message: String,
        param: Option<String>,
        r#type: String,
    ) -> Error {
        Error { code, message, param, r#type }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Error2 {
    /// A machine-readable error code that was returned.
    #[serde(rename = "code")]
    pub code: String,
    /// A human-readable description of the error that was returned.
    #[serde(rename = "message")]
    pub message: String,
}

impl Error2 {
    /// An error that occurred while generating the response.
    pub fn new(code: String, message: String) -> Error2 {
        Error2 { code, message }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "error")]
    pub error: Box<models::Error>,
}

impl ErrorResponse {
    pub fn new(error: models::Error) -> ErrorResponse {
        ErrorResponse { error: Box::new(error) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum IncludeEnum {
    #[serde(rename = "file_search_call.results")]
    #[default]
    FileSearchCallResults,
    #[serde(rename = "web_search_call.results")]
    WebSearchCallResults,
    #[serde(rename = "web_search_call.action.sources")]
    WebSearchCallActionSources,
    #[serde(rename = "message.input_image.image_url")]
    MessageInputImageImageUrl,
    #[serde(rename = "computer_call_output.output.image_url")]
    ComputerCallOutputOutputImageUrl,
    #[serde(rename = "code_interpreter_call.outputs")]
    CodeInterpreterCallOutputs,
    #[serde(rename = "reasoning.encrypted_content")]
    ReasoningEncryptedContent,
    #[serde(rename = "message.output_text.logprobs")]
    MessageOutputTextLogprobs,
}

impl std::fmt::Display for IncludeEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::FileSearchCallResults => write!(f, "file_search_call.results"),
            Self::WebSearchCallResults => write!(f, "web_search_call.results"),
            Self::WebSearchCallActionSources => write!(f, "web_search_call.action.sources"),
            Self::MessageInputImageImageUrl => write!(f, "message.input_image.image_url"),
            Self::ComputerCallOutputOutputImageUrl => {
                write!(f, "computer_call_output.output.image_url")
            }
            Self::CodeInterpreterCallOutputs => write!(f, "code_interpreter_call.outputs"),
            Self::ReasoningEncryptedContent => write!(f, "reasoning.encrypted_content"),
            Self::MessageOutputTextLogprobs => write!(f, "message.output_text.logprobs"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InferenceOptions {
    /// Preferred tool to invoke. Defaults to null when ChatKit should auto-select.
    #[serde(rename = "tool_choice", deserialize_with = "Option::deserialize")]
    pub tool_choice: Option<Box<models::ToolChoice>>,
    /// Model name that generated the response. Defaults to null when using the session default.
    #[serde(rename = "model", deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
}

impl InferenceOptions {
    /// Model and tool overrides applied when generating the assistant response.
    pub fn new(tool_choice: Option<models::ToolChoice>, model: Option<String>) -> InferenceOptions {
        InferenceOptions {
            tool_choice: tool_choice.map(Box::new),
            model,
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogProb {
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "logprob")]
    pub logprob: f64,
    #[serde(rename = "bytes")]
    pub bytes: Vec<i32>,
    #[serde(rename = "top_logprobs")]
    pub top_logprobs: Vec<models::TopLogProb>,
}

impl LogProb {
    /// The log probability of a token.
    pub fn new(
        token: String,
        logprob: f64,
        bytes: Vec<i32>,
        top_logprobs: Vec<models::TopLogProb>,
    ) -> LogProb {
        LogProb { token, logprob, bytes, top_logprobs }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogProbProperties {
    /// The token that was used to generate the log probability.
    #[serde(rename = "token")]
    pub token: String,
    /// The log probability of the token.
    #[serde(rename = "logprob")]
    pub logprob: f64,
    /// The bytes that were used to generate the log probability.
    #[serde(rename = "bytes")]
    pub bytes: Vec<i32>,
}

impl LogProbProperties {
    /// A log probability object.
    pub fn new(token: String, logprob: f64, bytes: Vec<i32>) -> LogProbProperties {
        LogProbProperties { token, logprob, bytes }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TopLogProb {
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "logprob")]
    pub logprob: f64,
    #[serde(rename = "bytes")]
    pub bytes: Vec<i32>,
}

impl TopLogProb {
    /// The top log probability of a token.
    pub fn new(token: String, logprob: f64, bytes: Vec<i32>) -> TopLogProb {
        TopLogProb { token, logprob, bytes }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ClientToolCallItemObject {
    #[serde(rename = "chatkit.thread_item")]
    #[default]
    ChatkitThreadItem,
}

/// Type discriminator that is always `chatkit.client_tool_call`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ClientToolCallItemType {
    #[serde(rename = "chatkit.client_tool_call")]
    #[default]
    ChatkitClientToolCall,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum OrderEnum {
    #[serde(rename = "asc")]
    #[default]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}

impl std::fmt::Display for OrderEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Asc => write!(f, "asc"),
            Self::Desc => write!(f, "desc"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct OtherChunkingStrategyResponseParam {
    /// Always `other`.
    #[serde(rename = "type")]
    pub r#type: OtherChunkingStrategyResponseParamType,
}

impl OtherChunkingStrategyResponseParam {
    /// This is returned when the chunking strategy is unknown. Typically, this is because the file was indexed before the `chunking_strategy` concept was introduced in the API.
    pub fn new(
        r#type: OtherChunkingStrategyResponseParamType,
    ) -> OtherChunkingStrategyResponseParam {
        OtherChunkingStrategyResponseParam { r#type }
    }
}
/// Always `other`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum OtherChunkingStrategyResponseParamType {
    #[serde(rename = "other")]
    #[default]
    Other,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    /// The identifier, which can be referenced in API endpoints
    #[serde(rename = "id")]
    pub id: String,
    /// The object type, which is always `organization.project`
    #[serde(rename = "object")]
    pub object: ProjectObject,
    /// The Unix timestamp (in seconds) of when the project was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// The name of the project. This appears in reporting.
    #[serde(
        rename = "name",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<Option<String>>,
    /// The Unix timestamp (in seconds) of when the project was archived or `null`.
    #[serde(
        rename = "archived_at",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub archived_at: Option<Option<i32>>,
    /// `active` or `archived`
    #[serde(
        rename = "status",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub status: Option<Option<String>>,
    /// The external key associated with the project.
    #[serde(
        rename = "external_key_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_key_id: Option<Option<String>>,
}

impl Project {
    /// Represents an individual project.
    pub fn new(id: String, object: ProjectObject, created_at: i32) -> Project {
        Project {
            id,
            object,
            created_at,
            name: None,
            archived_at: None,
            status: None,
            external_key_id: None,
        }
    }
}
/// The object type, which is always `organization.project`
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ProjectObject {
    #[serde(rename = "organization.project")]
    #[default]
    OrganizationProject,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    /// The unique identifier of the prompt template to use.
    #[serde(rename = "id")]
    pub id: String,
    /// Optional version of the prompt template.
    #[serde(
        rename = "version",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub version: Option<Option<String>>,
    /// Optional map of values to substitute in for variables in your prompt. The substitution values can either be strings, or other Response input types like images or files.
    #[serde(
        rename = "variables",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variables:
        Option<Option<std::collections::HashMap<String, models::ResponsePromptVariablesValue>>>,
}

impl Prompt {
    /// Reference to a prompt template and its variables. [Learn more](/docs/guides/text?api-mode=responses#reusable-prompts).
    pub fn new(id: String) -> Prompt {
        Prompt { id, version: None, variables: None }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum PromptCacheRetentionEnum {
    #[serde(rename = "in_memory")]
    #[default]
    InMemory,
    #[serde(rename = "24h")]
    Variant24h,
}

impl std::fmt::Display for PromptCacheRetentionEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InMemory => write!(f, "in_memory"),
            Self::Variant24h => write!(f, "24h"),
        }
    }
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ServiceTier {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "default")]
    Default,
    #[serde(rename = "flex")]
    Flex,
    #[serde(rename = "scale")]
    Scale,
    #[serde(rename = "priority")]
    Priority,
}

impl std::fmt::Display for ServiceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Default => write!(f, "default"),
            Self::Flex => write!(f, "flex"),
            Self::Scale => write!(f, "scale"),
            Self::Priority => write!(f, "priority"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpecificApplyPatchParam {
    /// The tool to call. Always `apply_patch`.
    #[serde(rename = "type")]
    pub r#type: SpecificApplyPatchParamType,
}

impl SpecificApplyPatchParam {
    /// Forces the model to call the apply_patch tool when executing a tool call.
    pub fn new(r#type: SpecificApplyPatchParamType) -> SpecificApplyPatchParam {
        SpecificApplyPatchParam { r#type }
    }
}
/// The tool to call. Always `apply_patch`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SpecificApplyPatchParamType {
    #[serde(rename = "apply_patch")]
    #[default]
    ApplyPatch,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpecificFunctionShellParam {
    /// The tool to call. Always `shell`.
    #[serde(rename = "type")]
    pub r#type: SpecificFunctionShellParamType,
}

impl SpecificFunctionShellParam {
    /// Forces the model to call the shell tool when a tool call is required.
    pub fn new(r#type: SpecificFunctionShellParamType) -> SpecificFunctionShellParam {
        SpecificFunctionShellParam { r#type }
    }
}
/// The tool to call. Always `shell`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SpecificFunctionShellParamType {
    #[serde(rename = "shell")]
    #[default]
    Shell,
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopConfiguration {
    String(String),
    ArrayVecString(Vec<String>),
}

impl Default for StopConfiguration {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeParam {
    /// Specifies the event type. For a type action, this property is always set to `type`.
    #[serde(rename = "type")]
    pub r#type: TypeParamType,
    /// The text to type.
    #[serde(rename = "text")]
    pub text: String,
}

impl TypeParam {
    /// An action to type in text.
    pub fn new(r#type: TypeParamType, text: String) -> TypeParam {
        TypeParam { r#type, text }
    }
}
/// Specifies the event type. For a type action, this property is always set to `type`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum TypeParamType {
    #[serde(rename = "type")]
    #[default]
    TypeParamType,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum Verbosity {
    #[serde(rename = "low")]
    #[default]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
}

impl std::fmt::Display for Verbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WidgetMessageItem {
    /// Identifier of the thread item.
    #[serde(rename = "id")]
    pub id: String,
    /// Type discriminator that is always `chatkit.thread_item`.
    #[serde(rename = "object")]
    pub object: WidgetMessageItemObject,
    /// Unix timestamp (in seconds) for when the item was created.
    #[serde(rename = "created_at")]
    pub created_at: i32,
    /// Identifier of the parent thread.
    #[serde(rename = "thread_id")]
    pub thread_id: String,
    /// Type discriminator that is always `chatkit.widget`.
    #[serde(rename = "type")]
    pub r#type: WidgetMessageItemType,
    /// Serialized widget payload rendered in the UI.
    #[serde(rename = "widget")]
    pub widget: String,
}

impl WidgetMessageItem {
    /// Thread item that renders a widget payload.
    pub fn new(
        id: String,
        object: WidgetMessageItemObject,
        created_at: i32,
        thread_id: String,
        r#type: WidgetMessageItemType,
        widget: String,
    ) -> WidgetMessageItem {
        WidgetMessageItem { id, object, created_at, thread_id, r#type, widget }
    }
}
/// Type discriminator that is always `chatkit.thread_item`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WidgetMessageItemObject {
    #[serde(rename = "chatkit.thread_item")]
    #[default]
    ChatkitThreadItem,
}

/// Type discriminator that is always `chatkit.widget`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum WidgetMessageItemType {
    #[serde(rename = "chatkit.widget")]
    #[default]
    ChatkitWidget,
}

