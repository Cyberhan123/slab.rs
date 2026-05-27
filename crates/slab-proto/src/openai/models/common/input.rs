use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputContent {
    #[serde(rename = "InputTextContent")]
    InputTextContent(Box<models::InputTextContent>),
    #[serde(rename = "InputImageContent")]
    InputImageContent(Box<models::InputImageContent>),
    #[serde(rename = "InputFileContent")]
    InputFileContent(Box<models::InputFileContent>),
}

impl Default for InputContent {
    fn default() -> Self {
        Self::InputTextContent(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputTextContent {
    /// The type of the input item. Always `input_text`.
    #[serde(rename = "type")]
    pub r#type: InputTextContentType,
    /// The text input to the model.
    #[serde(rename = "text")]
    pub text: String,
}

impl InputTextContent {
    /// A text input to the model.
    pub fn new(r#type: InputTextContentType, text: String) -> InputTextContent {
        InputTextContent { r#type, text }
    }
}
/// The type of the input item. Always `input_text`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputTextContentType {
    #[serde(rename = "input_text")]
    #[default]
    InputText,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputTextContentParam {
    /// The type of the input item. Always `input_text`.
    #[serde(rename = "type")]
    pub r#type: InputTextContentParamType,
    /// The text input to the model.
    #[serde(rename = "text")]
    pub text: String,
}

impl InputTextContentParam {
    /// A text input to the model.
    pub fn new(r#type: InputTextContentParamType, text: String) -> InputTextContentParam {
        InputTextContentParam { r#type, text }
    }
}
/// The type of the input item. Always `input_text`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputTextContentParamType {
    #[serde(rename = "input_text")]
    #[default]
    InputText,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFileContent {
    /// The type of the input item. Always `input_file`.
    #[serde(rename = "type")]
    pub r#type: InputFileContentType,
    /// The ID of the file to be sent to the model.
    #[serde(
        rename = "file_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_id: Option<Option<String>>,
    /// The name of the file to be sent to the model.
    #[serde(rename = "filename", skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// The content of the file to be sent to the model.
    #[serde(rename = "file_data", skip_serializing_if = "Option::is_none")]
    pub file_data: Option<String>,
    /// The URL of the file to be sent to the model.
    #[serde(rename = "file_url", skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
    /// The detail level of the file to be sent to the model. Use `low` for the default rendering behavior, or `high` to render the file at higher quality. Defaults to `low`.
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    pub detail: Option<models::FileInputDetail>,
}

impl InputFileContent {
    /// A file input to the model.
    pub fn new(r#type: InputFileContentType) -> InputFileContent {
        InputFileContent {
            r#type,
            file_id: None,
            filename: None,
            file_data: None,
            file_url: None,
            detail: None,
        }
    }
}
/// The type of the input item. Always `input_file`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputFileContentType {
    #[serde(rename = "input_file")]
    #[default]
    InputFile,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFileContentParam {
    /// The type of the input item. Always `input_file`.
    #[serde(rename = "type")]
    pub r#type: InputFileContentParamType,
    /// The ID of the file to be sent to the model.
    #[serde(
        rename = "file_id",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_id: Option<Option<String>>,
    /// The name of the file to be sent to the model.
    #[serde(
        rename = "filename",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub filename: Option<Option<String>>,
    /// The base64-encoded data of the file to be sent to the model.
    #[serde(
        rename = "file_data",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_data: Option<Option<String>>,
    /// The URL of the file to be sent to the model.
    #[serde(
        rename = "file_url",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub file_url: Option<Option<String>>,
    /// The detail level of the file to be sent to the model. Use `low` for the default rendering behavior, or `high` to render the file at higher quality. Defaults to `low`.
    #[serde(rename = "detail", skip_serializing_if = "Option::is_none")]
    pub detail: Option<models::FileDetailEnum>,
}

impl InputFileContentParam {
    /// A file input to the model.
    pub fn new(r#type: InputFileContentParamType) -> InputFileContentParam {
        InputFileContentParam {
            r#type,
            file_id: None,
            filename: None,
            file_data: None,
            file_url: None,
            detail: None,
        }
    }
}
/// The type of the input item. Always `input_file`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputFileContentParamType {
    #[serde(rename = "input_file")]
    #[default]
    InputFile,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputFidelity {
    #[serde(rename = "high")]
    #[default]
    High,
    #[serde(rename = "low")]
    Low,
}

impl std::fmt::Display for InputFidelity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Low => write!(f, "low"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputItem {
    #[serde(rename = "EasyInputMessage")]
    EasyInputMessage(Box<models::EasyInputMessage>),
    /// An item representing part of the context for the response to be generated by the model. Can contain text, images, and audio inputs, as well as previous assistant responses and tool call outputs.
    #[serde(rename = "one_of_1")]
    Item(Box<serde_json::Value>),
    #[serde(rename = "ItemReferenceParam")]
    ItemReferenceParam(Box<models::ItemReferenceParam>),
}

impl Default for InputItem {
    fn default() -> Self {
        Self::EasyInputMessage(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputMessage {
    /// The role of the message input. One of `user`, `system`, or `developer`.
    #[serde(rename = "role")]
    pub role: InputMessageRole,
    /// A list of one or many input items to the model, containing different content  types.
    #[serde(rename = "content")]
    pub content: Vec<models::InputContent>,
    /// The type of the message input. Always set to `message`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<InputMessageType>,
    /// The status of item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<InputMessageStatus>,
}

impl InputMessage {
    /// A message input to the model with a role indicating instruction following hierarchy. Instructions given with the `developer` or `system` role take precedence over instructions given with the `user` role.
    pub fn new(role: InputMessageRole, content: Vec<models::InputContent>) -> InputMessage {
        InputMessage { role, content, r#type: None, status: None }
    }
}
/// The role of the message input. One of `user`, `system`, or `developer`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageRole {
    #[serde(rename = "user")]
    #[default]
    User,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "developer")]
    Developer,
}

/// The type of the message input. Always set to `message`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageType {
    #[serde(rename = "message")]
    #[default]
    Message,
}

/// The status of item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageStatus {
    #[serde(rename = "in_progress")]
    #[default]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputMessageResource {
    /// The role of the message input. One of `user`, `system`, or `developer`.
    #[serde(rename = "role")]
    pub role: InputMessageResourceRole,
    /// A list of one or many input items to the model, containing different content  types.
    #[serde(rename = "content")]
    pub content: Vec<models::InputContent>,
    /// The unique ID of the message input.
    #[serde(rename = "id")]
    pub id: String,
    /// The type of the message input. Always set to `message`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<InputMessageResourceType>,
    /// The status of item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.
    #[serde(rename = "status", skip_serializing_if = "Option::is_none")]
    pub status: Option<InputMessageResourceStatus>,
}

impl InputMessageResource {
    pub fn new(
        role: InputMessageResourceRole,
        content: Vec<models::InputContent>,
        id: String,
    ) -> InputMessageResource {
        InputMessageResource { role, content, id, r#type: None, status: None }
    }
}
/// The role of the message input. One of `user`, `system`, or `developer`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageResourceRole {
    #[serde(rename = "user")]
    #[default]
    User,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "developer")]
    Developer,
}

/// The type of the message input. Always set to `message`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageResourceType {
    #[serde(rename = "message")]
    #[default]
    Message,
}

/// The status of item. One of `in_progress`, `completed`, or `incomplete`. Populated when items are returned via API.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessageResourceStatus {
    #[serde(rename = "in_progress")]
    #[default]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "incomplete")]
    Incomplete,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputMessagesItemReference {
    /// The type of input messages. Always `item_reference`.
    #[serde(rename = "type")]
    pub r#type: InputMessagesItemReferenceType,
    /// A reference to a variable in the `item` namespace. Ie, \"item.name\"
    #[serde(rename = "item_reference")]
    pub item_reference: String,
}

impl InputMessagesItemReference {
    pub fn new(
        r#type: InputMessagesItemReferenceType,
        item_reference: String,
    ) -> InputMessagesItemReference {
        InputMessagesItemReference { r#type, item_reference }
    }
}
/// The type of input messages. Always `item_reference`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessagesItemReferenceType {
    #[serde(rename = "item_reference")]
    #[default]
    ItemReference,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputMessagesTemplate {
    /// The type of input messages. Always `template`.
    #[serde(rename = "type")]
    pub r#type: InputMessagesTemplateType,
    /// A list of chat messages forming the prompt or context. May include variable references to the `item` namespace, ie {{item.name}}.
    #[serde(rename = "template")]
    pub template: Vec<models::InputMessagesTemplateTemplateInner>,
}

impl InputMessagesTemplate {
    pub fn new(
        r#type: InputMessagesTemplateType,
        template: Vec<models::InputMessagesTemplateTemplateInner>,
    ) -> InputMessagesTemplate {
        InputMessagesTemplate { r#type, template }
    }
}
/// The type of input messages. Always `template`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum InputMessagesTemplateType {
    #[serde(rename = "template")]
    #[default]
    Template,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateInputMessages {
    /// The type of input messages. Always `template`.
    #[serde(rename = "type")]
    pub r#type: TemplateInputMessagesType,
    /// A list of chat messages forming the prompt or context. May include variable references to the `item` namespace, ie {{item.name}}.
    #[serde(rename = "template")]
    pub template: Vec<models::TemplateInputMessagesTemplateInner>,
}

impl TemplateInputMessages {
    pub fn new(
        r#type: TemplateInputMessagesType,
        template: Vec<models::TemplateInputMessagesTemplateInner>,
    ) -> TemplateInputMessages {
        TemplateInputMessages { r#type, template }
    }
}
/// The type of input messages. Always `template`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum TemplateInputMessagesType {
    #[serde(rename = "template")]
    #[default]
    Template,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemReferenceInputMessages {
    /// The type of input messages. Always `item_reference`.
    #[serde(rename = "type")]
    pub r#type: ItemReferenceInputMessagesType,
    /// A reference to a variable in the `item` namespace. Ie, \"item.input_trajectory\"
    #[serde(rename = "item_reference")]
    pub item_reference: String,
}

impl ItemReferenceInputMessages {
    pub fn new(
        r#type: ItemReferenceInputMessagesType,
        item_reference: String,
    ) -> ItemReferenceInputMessages {
        ItemReferenceInputMessages { r#type, item_reference }
    }
}
/// The type of input messages. Always `item_reference`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ItemReferenceInputMessagesType {
    #[serde(rename = "item_reference")]
    #[default]
    ItemReference,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemReferenceParam {
    /// The ID of the item to reference.
    #[serde(rename = "id")]
    pub id: String,
    /// The type of item to reference. Always `item_reference`.
    #[serde(
        rename = "type",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub r#type: Option<Option<ItemReferenceParamType>>,
}

impl ItemReferenceParam {
    /// An internal identifier for an item to reference.
    pub fn new(id: String) -> ItemReferenceParam {
        ItemReferenceParam { id, r#type: None }
    }
}
/// The type of item to reference. Always `item_reference`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ItemReferenceParamType {
    #[serde(rename = "item_reference")]
    #[default]
    ItemReference,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleInputMessage {
    /// The role of the message (e.g. \"system\", \"assistant\", \"user\").
    #[serde(rename = "role")]
    pub role: String,
    /// The content of the message.
    #[serde(rename = "content")]
    pub content: String,
}

impl SimpleInputMessage {
    pub fn new(role: String, content: String) -> SimpleInputMessage {
        SimpleInputMessage { role, content }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputParam {
    /// A text input to the model, equivalent to a text input with the `user` role.
    TextInput(String),
    /// A list of one or many input items to the model, containing different content types.
    InputItemList(Vec<models::InputItem>),
}

impl Default for InputParam {
    fn default() -> Self {
        Self::TextInput(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextManagementParam {
    /// The context management entry type. Currently only 'compaction' is supported.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Token threshold at which compaction should be triggered for this entry.
    #[serde(
        rename = "compact_threshold",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub compact_threshold: Option<Option<i32>>,
}

impl ContextManagementParam {
    pub fn new(r#type: String) -> ContextManagementParam {
        ContextManagementParam { r#type, compact_threshold: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryParam {
    /// Enables chat users to access previous ChatKit threads. Defaults to true.
    #[serde(rename = "enabled", skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Number of recent ChatKit threads users have access to. Defaults to unlimited when unset.
    #[serde(rename = "recent_threads", skip_serializing_if = "Option::is_none")]
    pub recent_threads: Option<i32>,
}

impl HistoryParam {
    /// Controls how much historical context is retained for the session.
    pub fn new() -> HistoryParam {
        HistoryParam { enabled: None, recent_threads: None }
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowParam {
    /// Identifier for the workflow invoked by the session.
    #[serde(rename = "id")]
    pub id: String,
    /// Specific workflow version to run. Defaults to the latest deployed version.
    #[serde(rename = "version", skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// State variables forwarded to the workflow. Keys may be up to 64 characters, values must be primitive types, and the map defaults to an empty object.
    #[serde(rename = "state_variables", skip_serializing_if = "Option::is_none")]
    pub state_variables:
        Option<std::collections::HashMap<String, models::WorkflowParamStateVariablesValue>>,
    /// Optional tracing overrides for the workflow invocation. When omitted, tracing is enabled by default.
    #[serde(rename = "tracing", skip_serializing_if = "Option::is_none")]
    pub tracing: Option<Box<models::WorkflowTracingParam>>,
}

impl WorkflowParam {
    /// Workflow reference and overrides applied to the chat session.
    pub fn new(id: String) -> WorkflowParam {
        WorkflowParam { id, version: None, state_variables: None, tracing: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowParamStateVariablesValue {
    String(String),
    Integer(i32),
    Boolean(bool),
    Number(f64),
}

impl Default for WorkflowParamStateVariablesValue {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowTracingParam {
    /// Whether tracing is enabled during the session. Defaults to true.
    #[serde(rename = "enabled", skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl WorkflowTracingParam {
    /// Controls diagnostic tracing during the session.
    pub fn new() -> WorkflowTracingParam {
        WorkflowTracingParam { enabled: None }
    }
}
