//! Request and response schemas for the `/v1/agents/responses` route.

use serde::{Deserialize, Serialize};
use slab_agent::config::{
    AgentConfig, AgentToolChoice, MAX_INVALID_TOOL_CALL_RETRIES, MAX_TOOL_CONCURRENCY,
};
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
use slab_types::{ConversationMessage, I18nPayload, agent::AgentThreadStatus};
use utoipa::ToSchema;
use validator::{Validate, ValidationError, ValidationErrors};

use crate::domain::models::{
    AgentCommand, AgentCommandAction, AgentCommandResult, AgentCommandStatus,
    StructuredOutput as DomainStructuredOutput,
    StructuredOutputJsonSchema as DomainStructuredOutputJsonSchema,
};
use crate::schemas::chat::{ChatReasoningEffort, ChatToolCall, ChatVerbosity};

/// Agent configuration provided by the caller.
#[derive(Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct AgentConfigInput {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub max_tokens: Option<u32>,
    pub token_budget: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
    pub allowed_tools: Option<Vec<String>>,
    pub tool_choice: Option<AgentToolChoiceInput>,
    pub tool_concurrency: Option<u8>,
    pub invalid_tool_call_retries: Option<u8>,
    pub structured_output: Option<AgentStructuredOutputInput>,
    pub transient: Option<bool>,
}

impl From<AgentConfigInput> for AgentConfig {
    fn from(v: AgentConfigInput) -> Self {
        let defaults = AgentConfig::default();
        Self {
            model: v.model.unwrap_or(defaults.model),
            system_prompt: v.system_prompt,
            max_turns: v.max_turns.unwrap_or(defaults.max_turns),
            max_depth: defaults.max_depth,
            max_threads: defaults.max_threads,
            max_tokens: v.max_tokens,
            token_budget: v.token_budget,
            temperature: v.temperature,
            top_p: v.top_p,
            top_k: v.top_k,
            min_p: v.min_p,
            presence_penalty: v.presence_penalty,
            repetition_penalty: v.repetition_penalty,
            reasoning_effort: v.reasoning_effort.map(Into::into),
            verbosity: v.verbosity.map(Into::into),
            allowed_tools: v.allowed_tools.unwrap_or_default(),
            tool_choice: v.tool_choice.map(Into::into).unwrap_or_default(),
            tool_concurrency: v
                .tool_concurrency
                .unwrap_or(defaults.tool_concurrency)
                .clamp(1, MAX_TOOL_CONCURRENCY),
            invalid_tool_call_retries: v
                .invalid_tool_call_retries
                .unwrap_or(defaults.invalid_tool_call_retries)
                .clamp(0, MAX_INVALID_TOOL_CALL_RETRIES),
            structured_output: v.structured_output.map(Into::into),
            transient: v.transient.unwrap_or(defaults.transient),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentToolChoiceInput {
    Auto,
    None,
    Required,
    Tool { name: String },
}

impl From<AgentToolChoiceInput> for AgentToolChoice {
    fn from(value: AgentToolChoiceInput) -> Self {
        match value {
            AgentToolChoiceInput::Auto => Self::Auto,
            AgentToolChoiceInput::None => Self::None,
            AgentToolChoiceInput::Required => Self::Required,
            AgentToolChoiceInput::Tool { name } => Self::Tool { name },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentStructuredOutputInput {
    JsonObject,
    JsonSchema {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        strict: Option<bool>,
        schema: serde_json::Value,
    },
}

impl From<AgentStructuredOutputInput> for DomainStructuredOutput {
    fn from(value: AgentStructuredOutputInput) -> Self {
        match value {
            AgentStructuredOutputInput::JsonObject => Self::JsonObject,
            AgentStructuredOutputInput::JsonSchema { name, description, strict, schema } => {
                Self::JsonSchema(DomainStructuredOutputJsonSchema::new(
                    name,
                    description,
                    strict,
                    schema,
                ))
            }
        }
    }
}

/// A single message in the initial conversation.
#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
pub struct MessageInput {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ChatToolCall>,
}

impl From<MessageInput> for slab_types::ConversationMessage {
    fn from(v: MessageInput) -> Self {
        slab_types::ConversationMessage {
            role: v.role,
            content: slab_types::ConversationMessageContent::Text(v.content),
            name: v.name,
            tool_call_id: v.tool_call_id,
            tool_calls: v.tool_calls.into_iter().map(Into::into).collect(),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// OpenAI-Responses-canonical request body (`client.responses.create({...})`)
// ──────────────────────────────────────────────────────────────────────────

/// `POST /v1/agents/responses` body as sent by the official `openai` SDK
/// (`ResponseCreateParamsBase`). Slab translates `input` + a subset of config;
/// unknown fields are ignored (no `deny_unknown_fields`) so future SDK fields
/// don't break the server. `input` is held as a `serde_json::Value` (a string
/// or an array of input items) so the type is `ToSchema`-derivable.
#[derive(Debug, Clone, Deserialize, Default, ToSchema)]
pub struct OpenAICreateRequest {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub reasoning: Option<OpenAIReasoningInput>,
    #[serde(default)]
    pub text: Option<OpenAITextInput>,
}

#[derive(Debug, Clone, Deserialize, Default, ToSchema)]
pub struct OpenAIReasoningInput {
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default, ToSchema)]
pub struct OpenAITextInput {
    #[serde(default)]
    pub format: Option<serde_json::Value>,
}

impl OpenAICreateRequest {
    /// Translate the OpenAI `input` into slab `MessageInput`s.
    pub fn to_messages(&self) -> Vec<MessageInput> {
        openai_input_to_messages(&self.input)
    }

    /// Translate the OpenAI request into slab's `AgentConfigInput`
    /// (then `AgentConfig::from(...)`).
    pub fn to_config_input(&self) -> AgentConfigInput {
        AgentConfigInput {
            model: self.model.clone(),
            system_prompt: self.instructions.clone(),
            max_tokens: self.max_output_tokens,
            temperature: self.temperature,
            top_p: self.top_p,
            reasoning_effort: self
                .reasoning
                .as_ref()
                .and_then(|r| r.effort.as_deref())
                .and_then(parse_reasoning_effort),
            tool_choice: self.tool_choice.as_ref().and_then(parse_tool_choice),
            structured_output: self
                .text
                .as_ref()
                .and_then(|t| t.format.as_ref())
                .and_then(parse_text_format),
            ..Default::default()
        }
    }

    /// Translate the OpenAI-compatible create request into the app-core agent
    /// command used by the application service.
    pub fn to_agent_command(&self, session_id: String) -> AgentCommand {
        let messages: Vec<ConversationMessage> =
            self.to_messages().into_iter().map(Into::into).collect();
        match self.previous_response_id.as_deref().filter(|value| !value.is_empty()) {
            Some(thread_id) => AgentCommand::AppendInput {
                request_id: None,
                thread_id: thread_id.to_owned(),
                content: last_user_text(&messages),
            },
            None => AgentCommand::CreateResponse {
                request_id: None,
                session_id,
                config: self.to_config_input().into(),
                messages,
            },
        }
    }
}

fn last_user_text(messages: &[ConversationMessage]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.rendered_text())
        .unwrap_or_default()
}

fn openai_input_to_messages(input: &serde_json::Value) -> Vec<MessageInput> {
    match input {
        serde_json::Value::String(text) => {
            vec![MessageInput {
                role: "user".to_owned(),
                content: text.clone(),
                ..Default::default()
            }]
        }
        serde_json::Value::Array(items) => {
            let mut messages: Vec<MessageInput> = Vec::new();
            for item in items {
                let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("message");
                match kind {
                    "message" => {
                        let role =
                            item.get("role").and_then(|v| v.as_str()).unwrap_or("user").to_owned();
                        let content = render_message_content(item.get("content"));
                        // Skip empty assistant turns (a bare function_call follows it).
                        if !(role == "assistant" && content.trim().is_empty()) {
                            messages.push(MessageInput { role, content, ..Default::default() });
                        }
                    }
                    "function_call" => {
                        let call = ChatToolCall {
                            id: item.get("call_id").and_then(|v| v.as_str()).map(str::to_owned),
                            r#type: "function".to_owned(),
                            function: crate::schemas::chat::ChatToolFunction {
                                name: item
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_owned(),
                                arguments: item
                                    .get("arguments")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_owned(),
                            },
                        };
                        fold_tool_call(&mut messages, call);
                    }
                    "function_call_output" => {
                        let call_id =
                            item.get("call_id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                        let output = item.get("output").map(render_output).unwrap_or_default();
                        messages.push(MessageInput {
                            role: "tool".to_owned(),
                            content: output,
                            tool_call_id: Some(call_id),
                            ..Default::default()
                        });
                    }
                    // reasoning / computer / file_search / mcp / shell / apply_patch / etc.:
                    // slab has no carrier today; dropped.
                    _ => {}
                }
            }
            messages
        }
        _ => Vec::new(),
    }
}

/// Concatenate the `text` of `input_text` / `output_text` / `refusal` parts, or
/// return the bare string content.
fn render_message_content(content: Option<&serde_json::Value>) -> String {
    let Some(value) = content else {
        return String::new();
    };
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| {
                let t = part.get("type").and_then(|v| v.as_str())?;
                let text = part.get("text").and_then(|v| v.as_str())?;
                matches!(t, "input_text" | "output_text" | "refusal").then_some(text.to_owned())
            })
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn render_output(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// OpenAI represents an assistant turn as `[message, function_call, ...]` items;
/// slab models tool calls as fields on the assistant `MessageInput`. Fold a call
/// onto the most recent assistant message (creating one if absent).
fn fold_tool_call(messages: &mut Vec<MessageInput>, call: ChatToolCall) {
    let needs_new = messages.last().map(|m| m.role != "assistant").unwrap_or(true);
    if needs_new {
        messages.push(MessageInput {
            role: "assistant".to_owned(),
            content: String::new(),
            ..Default::default()
        });
    }
    messages.last_mut().expect("assistant message").tool_calls.push(call);
}

fn parse_reasoning_effort(raw: &str) -> Option<ChatReasoningEffort> {
    Some(match raw {
        "none" => ChatReasoningEffort::None,
        "low" => ChatReasoningEffort::Low,
        "medium" => ChatReasoningEffort::Medium,
        "high" => ChatReasoningEffort::High,
        "minimal" => ChatReasoningEffort::Minimal,
        _ => return None,
    })
}

fn parse_tool_choice(value: &serde_json::Value) -> Option<AgentToolChoiceInput> {
    match value {
        serde_json::Value::String(s) => match s.as_str() {
            "auto" => Some(AgentToolChoiceInput::Auto),
            "none" => Some(AgentToolChoiceInput::None),
            "required" => Some(AgentToolChoiceInput::Required),
            _ => None,
        },
        serde_json::Value::Object(obj) => {
            let name = obj.get("name").and_then(|v| v.as_str()).map(str::to_owned)?;
            Some(AgentToolChoiceInput::Tool { name })
        }
        _ => None,
    }
}

fn parse_text_format(value: &serde_json::Value) -> Option<AgentStructuredOutputInput> {
    let obj = value.as_object()?;
    let kind = obj.get("type").and_then(|v| v.as_str())?;
    match kind {
        "json_object" => Some(AgentStructuredOutputInput::JsonObject),
        "json_schema" => Some(AgentStructuredOutputInput::JsonSchema {
            name: obj.get("name").and_then(|v| v.as_str()).map(str::to_owned),
            description: obj.get("description").and_then(|v| v.as_str()).map(str::to_owned),
            strict: obj.get("strict").and_then(|v| v.as_bool()),
            schema: obj.get("schema").cloned().unwrap_or(serde_json::Value::Null),
        }),
        _ => None,
    }
}

/// Client message accepted by `GET` WebSocket and `POST /v1/agents/responses`.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum AgentResponsesClientMessage {
    #[serde(rename = "agent.session.restore")]
    SessionRestore {
        #[serde(default)]
        request_id: Option<String>,
        session_id: String,
    },
    #[serde(rename = "agent.response.create")]
    ResponseCreate {
        #[serde(default)]
        request_id: Option<String>,
        session_id: String,
        #[serde(default)]
        config: Box<AgentConfigInput>,
        #[serde(default)]
        messages: Vec<MessageInput>,
    },
    #[serde(rename = "agent.input")]
    Input {
        #[serde(default)]
        request_id: Option<String>,
        thread_id: String,
        content: String,
    },
    #[serde(rename = "agent.approval.resolve")]
    ApprovalResolve {
        #[serde(default)]
        request_id: Option<String>,
        thread_id: String,
        call_id: String,
        approved: bool,
    },
    #[serde(rename = "agent.interrupt")]
    Interrupt {
        #[serde(default)]
        request_id: Option<String>,
        thread_id: String,
    },
    #[serde(rename = "agent.shutdown")]
    Shutdown {
        #[serde(default)]
        request_id: Option<String>,
        thread_id: String,
    },
}

impl AgentResponsesClientMessage {
    pub fn action(&self) -> AgentResponsesAction {
        match self {
            Self::SessionRestore { .. } => AgentResponsesAction::SessionRestore,
            Self::ResponseCreate { .. } => AgentResponsesAction::ResponseCreate,
            Self::Input { .. } => AgentResponsesAction::Input,
            Self::ApprovalResolve { .. } => AgentResponsesAction::ApprovalResolve,
            Self::Interrupt { .. } => AgentResponsesAction::Interrupt,
            Self::Shutdown { .. } => AgentResponsesAction::Shutdown,
        }
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::SessionRestore { request_id, .. }
            | Self::ResponseCreate { request_id, .. }
            | Self::Input { request_id, .. }
            | Self::ApprovalResolve { request_id, .. }
            | Self::Interrupt { request_id, .. }
            | Self::Shutdown { request_id, .. } => request_id.as_deref(),
        }
    }
}

impl From<AgentResponsesClientMessage> for AgentCommand {
    fn from(message: AgentResponsesClientMessage) -> Self {
        match message {
            AgentResponsesClientMessage::SessionRestore { request_id, session_id } => {
                Self::RestoreSession { request_id, session_id }
            }
            AgentResponsesClientMessage::ResponseCreate {
                request_id,
                session_id,
                config,
                messages,
            } => Self::CreateResponse {
                request_id,
                session_id,
                config: (*config).into(),
                messages: messages.into_iter().map(Into::into).collect(),
            },
            AgentResponsesClientMessage::Input { request_id, thread_id, content } => {
                Self::AppendInput { request_id, thread_id, content }
            }
            AgentResponsesClientMessage::ApprovalResolve {
                request_id,
                thread_id,
                call_id,
                approved,
            } => Self::ResolveApproval { request_id, thread_id, call_id, approved },
            AgentResponsesClientMessage::Interrupt { request_id, thread_id } => {
                Self::Interrupt { request_id, thread_id }
            }
            AgentResponsesClientMessage::Shutdown { request_id, thread_id } => {
                Self::Shutdown { request_id, thread_id }
            }
        }
    }
}

impl Validate for AgentResponsesClientMessage {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::SessionRestore { session_id, .. } => {
                add_non_blank(&mut errors, "session_id", session_id);
            }
            Self::ResponseCreate { session_id, config, messages, .. } => {
                add_non_blank(&mut errors, "session_id", session_id);
                validate_agent_config(&mut errors, config);
                for message in messages {
                    add_non_blank(&mut errors, "role", &message.role);
                }
            }
            Self::Input { thread_id, content, .. } => {
                add_non_blank(&mut errors, "thread_id", thread_id);
                add_non_blank(&mut errors, "content", content);
            }
            Self::ApprovalResolve { thread_id, call_id, .. } => {
                add_non_blank(&mut errors, "thread_id", thread_id);
                add_non_blank(&mut errors, "call_id", call_id);
            }
            Self::Interrupt { thread_id, .. } | Self::Shutdown { thread_id, .. } => {
                add_non_blank(&mut errors, "thread_id", thread_id);
            }
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

fn add_non_blank(errors: &mut ValidationErrors, field: &'static str, value: &str) {
    if !value.trim().is_empty() {
        return;
    }

    let mut error = ValidationError::new("required");
    error.message = Some(format!("{field} must not be blank").into());
    errors.add(field, error);
}

fn validate_agent_config(errors: &mut ValidationErrors, config: &AgentConfigInput) {
    if let Some(0) = config.tool_concurrency {
        add_field_error(errors, "tool_concurrency", "tool_concurrency must be at least 1");
    }
    if config.tool_concurrency.is_some_and(|value| value > MAX_TOOL_CONCURRENCY) {
        add_field_error(errors, "tool_concurrency", "tool_concurrency must be at most 4");
    }
    if config.invalid_tool_call_retries.is_some_and(|value| value > MAX_INVALID_TOOL_CALL_RETRIES) {
        add_field_error(
            errors,
            "invalid_tool_call_retries",
            "invalid_tool_call_retries must be at most 3",
        );
    }
    if let Some(AgentToolChoiceInput::Tool { name }) = &config.tool_choice {
        add_non_blank(errors, "tool_choice.name", name);
    }
    if let Some(allowed_tools) = &config.allowed_tools {
        for tool_name in allowed_tools {
            add_non_blank(errors, "allowed_tools", tool_name);
        }
    }
}

fn add_field_error(errors: &mut ValidationErrors, field: &'static str, message: &'static str) {
    let mut error = ValidationError::new("range");
    error.message = Some(message.into());
    errors.add(field, error);
}

/// Client action acknowledged by `/v1/agents/responses`.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentResponsesAction {
    SessionRestore,
    ResponseCreate,
    Input,
    ApprovalResolve,
    Interrupt,
    Shutdown,
}

impl From<AgentCommandAction> for AgentResponsesAction {
    fn from(action: AgentCommandAction) -> Self {
        match action {
            AgentCommandAction::RestoreSession => Self::SessionRestore,
            AgentCommandAction::CreateResponse => Self::ResponseCreate,
            AgentCommandAction::AppendInput => Self::Input,
            AgentCommandAction::ResolveApproval => Self::ApprovalResolve,
            AgentCommandAction::Interrupt => Self::Interrupt,
            AgentCommandAction::Shutdown => Self::Shutdown,
        }
    }
}

/// Server message returned by `POST /v1/agents/responses` and emitted on the
/// WebSocket control channel. Agent response events are sent as raw
/// `AgentStreamEvent` frames.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type")]
pub enum AgentResponsesServerMessage {
    #[serde(rename = "agent.ack")]
    Ack {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        action: AgentResponsesAction,
        accepted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<AgentStatusValue>,
        #[serde(skip_serializing_if = "Option::is_none")]
        delivered: Option<bool>,
    },
    #[serde(rename = "agent.session.restored")]
    SessionRestored {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        session_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread: Option<AgentThreadResponse>,
        /// Legacy per-message history (chat-completion shape). Retained for
        /// backward compatibility while the frontend migrates to `responses`.
        messages: Vec<AgentThreadMessageResponse>,
        /// Complete OpenAI-Responses-canonical `Response` objects, one per agent
        /// run, oldest first. Serialized verbatim from
        /// `agent_thread_responses.response_json`.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        responses: Vec<serde_json::Value>,
    },
    #[serde(rename = "agent.error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        i18n: Option<I18nPayload>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
    },
}

impl From<AgentCommandResult> for AgentResponsesServerMessage {
    fn from(result: AgentCommandResult) -> Self {
        if let Some(session) = result.session {
            return Self::SessionRestored {
                request_id: result.request_id,
                session_id: session.session_id,
                thread: session.thread.map(Into::into),
                messages: session.messages.into_iter().map(Into::into).collect(),
                responses: session.responses,
            };
        }

        Self::Ack {
            request_id: result.request_id,
            action: result.action.into(),
            accepted: result.accepted,
            thread_id: result.thread_id,
            status: result.status.map(Into::into),
            delivered: result.delivered,
        }
    }
}

/// Persisted agent thread summary.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AgentThreadResponse {
    pub id: String,
    pub session_id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub status: AgentStatusValue,
    pub role_name: Option<String>,
    pub config_json: String,
    pub completion_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Persisted agent thread message.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AgentThreadMessageResponse {
    pub id: String,
    pub thread_id: String,
    pub turn_index: u32,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ChatToolCall>,
    pub created_at: String,
}

impl From<ThreadSnapshot> for AgentThreadResponse {
    fn from(thread: ThreadSnapshot) -> Self {
        Self {
            id: thread.id,
            session_id: thread.session_id,
            parent_id: thread.parent_id,
            depth: thread.depth,
            status: thread.status.into(),
            role_name: thread.role_name,
            config_json: thread.config_json,
            completion_text: thread.completion_text,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
        }
    }
}

impl From<ThreadMessageRecord> for AgentThreadMessageResponse {
    fn from(record: ThreadMessageRecord) -> Self {
        let message = record.message;
        let content = message.content.rendered_text();
        let tool_call_id = message.tool_call_id;
        let tool_calls = message.tool_calls.into_iter().map(Into::into).collect();
        Self {
            id: record.id,
            thread_id: record.thread_id,
            turn_index: record.turn_index,
            role: message.role,
            content,
            tool_call_id,
            tool_calls,
            created_at: record.created_at,
        }
    }
}

/// Serializable mirror of [`AgentThreadStatus`].
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusValue {
    Pending,
    Running,
    Interrupting,
    Interrupted,
    Completed,
    Errored,
    Shutdown,
}

impl From<AgentThreadStatus> for AgentStatusValue {
    fn from(s: AgentThreadStatus) -> Self {
        match s {
            AgentThreadStatus::Pending => Self::Pending,
            AgentThreadStatus::Running => Self::Running,
            AgentThreadStatus::Interrupting => Self::Interrupting,
            AgentThreadStatus::Interrupted => Self::Interrupted,
            AgentThreadStatus::Completed => Self::Completed,
            AgentThreadStatus::Errored => Self::Errored,
            AgentThreadStatus::Shutdown => Self::Shutdown,
        }
    }
}

impl From<AgentCommandStatus> for AgentStatusValue {
    fn from(status: AgentCommandStatus) -> Self {
        match status {
            AgentCommandStatus::Pending => Self::Pending,
            AgentCommandStatus::Interrupting => Self::Interrupting,
            AgentCommandStatus::Shutdown => Self::Shutdown,
        }
    }
}

/// Outcome of a workspace migration preparation (B-8 / INFRA-01): the project
/// id the snapshot was scoped to + how many agent threads were suspended.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkspaceMigrationResponse {
    pub project_id: String,
    pub suspended_count: u32,
}

impl From<crate::domain::services::agent::WorkspaceMigrationOutcome>
    for WorkspaceMigrationResponse
{
    fn from(outcome: crate::domain::services::agent::WorkspaceMigrationOutcome) -> Self {
        Self { project_id: outcome.project_id, suspended_count: outcome.suspended_count as u32 }
    }
}

#[cfg(test)]
mod tests {
    use slab_agent::{
        config::{AgentConfig, AgentToolChoice},
        port::ThreadMessageRecord,
    };
    use slab_types::{
        ConversationMessage, ConversationMessageContent, ConversationToolCall,
        ConversationToolFunction, StructuredOutput,
    };
    use validator::Validate;

    use super::*;

    #[test]
    fn openai_input_string_translates_to_single_user_message() {
        let req: OpenAICreateRequest =
            serde_json::from_str(r#"{"model":"m","input":"hello"}"#).unwrap();
        let messages = req.to_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
    }

    #[test]
    fn openai_input_array_groups_function_calls_into_assistant_message() {
        let req: OpenAICreateRequest = serde_json::from_str(
            r#"{"input":[
                {"type":"message","role":"user","content":"hi"},
                {"type":"message","role":"assistant","content":[{"type":"output_text","text":"thinking"}]},
                {"type":"function_call","call_id":"c1","name":"search","arguments":"{\"q\":\"x\"}"},
                {"type":"function_call_output","call_id":"c1","output":"result"}
            ]}"#,
        )
        .unwrap();
        let messages = req.to_messages();
        // user, assistant (with folded tool call), tool
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hi");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "thinking");
        assert_eq!(messages[1].tool_calls.len(), 1);
        assert_eq!(messages[1].tool_calls[0].function.name, "search");
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("c1"));
        assert_eq!(messages[2].content, "result");
    }

    #[test]
    fn openai_request_maps_to_agent_config_input() {
        let req: OpenAICreateRequest = serde_json::from_str(
            r#"{"model":"gpt-x","instructions":"be brief","max_output_tokens":128,
               "temperature":0.5,"reasoning":{"effort":"high"},
               "tool_choice":"required","text":{"format":{"type":"json_object"}}}"#,
        )
        .unwrap();
        let config = AgentConfig::from(req.to_config_input());
        assert_eq!(config.model, "gpt-x");
        assert_eq!(config.system_prompt.as_deref(), Some("be brief"));
        assert_eq!(config.max_tokens, Some(128));
        assert_eq!(config.temperature, Some(0.5));
        assert!(config.reasoning_effort.is_some());
        assert!(matches!(config.tool_choice, AgentToolChoice::Required));
        assert!(config.structured_output.is_some());
    }

    #[test]
    fn openai_create_request_maps_to_agent_create_command() {
        let req: OpenAICreateRequest =
            serde_json::from_str(r#"{"model":"gpt-x","instructions":"be brief","input":"hello"}"#)
                .unwrap();

        let command = req.to_agent_command("session-1".into());
        let AgentCommand::CreateResponse { request_id, session_id, config, messages } = command
        else {
            panic!("expected create response command");
        };

        assert_eq!(request_id, None);
        assert_eq!(session_id, "session-1");
        assert_eq!(config.model, "gpt-x");
        assert_eq!(config.system_prompt.as_deref(), Some("be brief"));
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content.rendered_text(), "hello");
    }

    #[test]
    fn openai_previous_response_maps_to_agent_append_input_command() {
        let req: OpenAICreateRequest = serde_json::from_str(
            r#"{"previous_response_id":"thread-1","input":[
                {"type":"message","role":"assistant","content":"old"},
                {"type":"message","role":"user","content":"new turn"}
            ]}"#,
        )
        .unwrap();

        let command = req.to_agent_command("session-1".into());
        let AgentCommand::AppendInput { request_id, thread_id, content } = command else {
            panic!("expected append input command");
        };

        assert_eq!(request_id, None);
        assert_eq!(thread_id, "thread-1");
        assert_eq!(content, "new turn");
    }

    #[test]
    fn slab_client_messages_map_to_agent_commands() {
        let restore = AgentCommand::from(AgentResponsesClientMessage::SessionRestore {
            request_id: Some("req-restore".into()),
            session_id: "session-1".into(),
        });
        assert!(matches!(
            restore,
            AgentCommand::RestoreSession { request_id: Some(ref id), session_id }
                if id == "req-restore" && session_id == "session-1"
        ));

        let create = AgentCommand::from(AgentResponsesClientMessage::ResponseCreate {
            request_id: Some("req-create".into()),
            session_id: "session-1".into(),
            config: Box::new(AgentConfigInput { model: Some("mock".into()), ..Default::default() }),
            messages: vec![MessageInput {
                role: "user".into(),
                content: "hello".into(),
                ..Default::default()
            }],
        });
        assert!(matches!(
            create,
            AgentCommand::CreateResponse {
                request_id: Some(ref id),
                session_id,
                ref config,
                ref messages,
            } if id == "req-create"
                && session_id == "session-1"
                && config.model == "mock"
                && messages.len() == 1
                && messages[0].content.rendered_text() == "hello"
        ));

        let input = AgentCommand::from(AgentResponsesClientMessage::Input {
            request_id: Some("req-input".into()),
            thread_id: "thread-1".into(),
            content: "next".into(),
        });
        assert!(matches!(
            input,
            AgentCommand::AppendInput { request_id: Some(ref id), thread_id, content }
                if id == "req-input" && thread_id == "thread-1" && content == "next"
        ));

        let approval = AgentCommand::from(AgentResponsesClientMessage::ApprovalResolve {
            request_id: Some("req-approval".into()),
            thread_id: "thread-1".into(),
            call_id: "call-1".into(),
            approved: true,
        });
        assert!(matches!(
            approval,
            AgentCommand::ResolveApproval {
                request_id: Some(ref id),
                thread_id,
                call_id,
                approved: true,
            } if id == "req-approval" && thread_id == "thread-1" && call_id == "call-1"
        ));

        let interrupt = AgentCommand::from(AgentResponsesClientMessage::Interrupt {
            request_id: Some("req-interrupt".into()),
            thread_id: "thread-1".into(),
        });
        assert!(matches!(
            interrupt,
            AgentCommand::Interrupt { request_id: Some(ref id), thread_id }
                if id == "req-interrupt" && thread_id == "thread-1"
        ));

        let shutdown = AgentCommand::from(AgentResponsesClientMessage::Shutdown {
            request_id: Some("req-shutdown".into()),
            thread_id: "thread-1".into(),
        });
        assert!(matches!(
            shutdown,
            AgentCommand::Shutdown { request_id: Some(ref id), thread_id }
                if id == "req-shutdown" && thread_id == "thread-1"
        ));
    }

    use crate::schemas::chat::{ChatToolCall, ChatToolFunction};

    use super::{
        AgentConfigInput, AgentResponsesClientMessage, AgentStructuredOutputInput,
        AgentThreadMessageResponse, AgentToolChoiceInput, MessageInput,
    };

    #[test]
    fn agent_config_input_maps_new_defaults_and_structured_output() {
        let config = AgentConfig::from(AgentConfigInput {
            model: Some("mock".into()),
            tool_choice: Some(AgentToolChoiceInput::Tool { name: "echo".into() }),
            tool_concurrency: Some(4),
            invalid_tool_call_retries: Some(3),
            structured_output: Some(AgentStructuredOutputInput::JsonObject),
            ..AgentConfigInput::default()
        });

        assert_eq!(config.model, "mock");
        assert_eq!(config.tool_choice, AgentToolChoice::Tool { name: "echo".into() });
        assert_eq!(config.token_budget, None);
        assert_eq!(config.tool_concurrency, 4);
        assert_eq!(config.invalid_tool_call_retries, 3);
        assert_eq!(config.structured_output, Some(StructuredOutput::JsonObject));
    }

    #[test]
    fn agent_config_input_rejects_out_of_range_tool_controls() {
        let message = AgentResponsesClientMessage::ResponseCreate {
            request_id: None,
            session_id: "session-1".into(),
            config: Box::new(AgentConfigInput {
                tool_concurrency: Some(5),
                invalid_tool_call_retries: Some(4),
                tool_choice: Some(AgentToolChoiceInput::Tool { name: " ".into() }),
                ..AgentConfigInput::default()
            }),
            messages: Vec::new(),
        };

        let errors = message.validate().expect_err("invalid config");
        assert!(errors.field_errors().contains_key("tool_concurrency"));
        assert!(errors.field_errors().contains_key("invalid_tool_call_retries"));
        assert!(errors.field_errors().contains_key("tool_choice.name"));
    }

    #[test]
    fn old_agent_config_json_deserializes_with_new_defaults() {
        let config = serde_json::from_value::<AgentConfig>(serde_json::json!({
            "model": "mock",
            "system_prompt": null,
            "max_turns": 10,
            "max_depth": 3,
            "max_threads": 8,
            "max_tokens": null,
            "token_budget": null,
            "temperature": null,
            "top_p": null,
            "top_k": null,
            "min_p": null,
            "presence_penalty": null,
            "repetition_penalty": null,
            "reasoning_effort": null,
            "verbosity": null
        }))
        .expect("old config json");

        assert_eq!(config.tool_choice, AgentToolChoice::Auto);
        assert_eq!(config.tool_concurrency, 1);
        assert_eq!(config.invalid_tool_call_retries, 1);
        assert_eq!(config.structured_output, None);
        assert_eq!(config.token_budget, None);
        assert!(!config.transient);
    }

    #[test]
    fn message_input_preserves_tool_role_metadata() {
        let message = slab_types::ConversationMessage::from(MessageInput {
            role: "tool".into(),
            content: "search result".into(),
            name: Some("web_search".into()),
            tool_call_id: Some("call-1".into()),
            tool_calls: vec![ChatToolCall {
                id: Some("call-2".into()),
                r#type: "function".into(),
                function: ChatToolFunction {
                    name: "echo".into(),
                    arguments: r#"{"message":"hello"}"#.into(),
                },
            }],
        });

        assert_eq!(message.role, "tool");
        assert_eq!(message.content.rendered_text(), "search result");
        assert!(message.rendered_text().contains("tool_call_id: call-1"));
        assert_eq!(message.name.as_deref(), Some("web_search"));
        assert_eq!(message.tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(message.tool_calls.len(), 1);
        assert_eq!(message.tool_calls[0].id.as_deref(), Some("call-2"));
    }

    #[test]
    fn agent_thread_message_response_preserves_assistant_tool_calls() {
        let response = AgentThreadMessageResponse::from(ThreadMessageRecord {
            id: "message-1".into(),
            thread_id: "thread-1".into(),
            turn_index: 0,
            message: ConversationMessage {
                role: "assistant".into(),
                content: ConversationMessageContent::Text(String::new()),
                name: None,
                tool_call_id: None,
                tool_calls: vec![ConversationToolCall {
                    id: Some("call-1".into()),
                    r#type: "function".into(),
                    function: ConversationToolFunction {
                        name: "web_search".into(),
                        arguments: r#"{"query":"Japan weather"}"#.into(),
                    },
                }],
            },
            created_at: "2026-01-01T00:00:00Z".into(),
        });

        assert_eq!(response.content, "");
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].id.as_deref(), Some("call-1"));
        assert_eq!(response.tool_calls[0].function.name, "web_search");
    }

    #[test]
    fn agent_thread_message_response_keeps_assistant_text() {
        let response = AgentThreadMessageResponse::from(ThreadMessageRecord {
            id: "message-1".into(),
            thread_id: "thread-1".into(),
            turn_index: 0,
            message: ConversationMessage {
                role: "assistant".into(),
                content: ConversationMessageContent::Text("Tokyo is sunny.".into()),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            },
            created_at: "2026-01-01T00:00:00Z".into(),
        });

        assert_eq!(response.content, "Tokyo is sunny.");
        assert!(response.tool_calls.is_empty());
    }
}
