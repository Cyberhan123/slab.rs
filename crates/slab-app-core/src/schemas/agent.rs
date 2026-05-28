//! Request and response schemas for the `/v1/agents/responses` route.

use serde::{Deserialize, Serialize};
use slab_agent::config::AgentConfig;
use slab_agent::port::{ThreadMessageRecord, ThreadSnapshot};
use slab_types::agent::AgentThreadStatus;
use utoipa::ToSchema;
use validator::{Validate, ValidationError, ValidationErrors};

use crate::schemas::chat::{ChatReasoningEffort, ChatVerbosity};

/// Agent configuration provided by the caller.
#[derive(Debug, Default, Deserialize, Serialize, ToSchema)]
pub struct AgentConfigInput {
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub min_p: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub repetition_penalty: Option<f32>,
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
    pub allowed_tools: Option<Vec<String>>,
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
            temperature: v.temperature,
            top_p: v.top_p,
            top_k: v.top_k,
            min_p: v.min_p,
            presence_penalty: v.presence_penalty,
            repetition_penalty: v.repetition_penalty,
            reasoning_effort: v.reasoning_effort.map(Into::into),
            verbosity: v.verbosity.map(Into::into),
            allowed_tools: v.allowed_tools.unwrap_or_default(),
        }
    }
}

/// A single message in the initial conversation.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct MessageInput {
    pub role: String,
    pub content: String,
}

impl From<MessageInput> for slab_types::ConversationMessage {
    fn from(v: MessageInput) -> Self {
        slab_types::ConversationMessage {
            role: v.role,
            content: slab_types::ConversationMessageContent::Text(v.content),
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }
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
        config: AgentConfigInput,
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

impl Validate for AgentResponsesClientMessage {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        match self {
            Self::SessionRestore { session_id, .. } => {
                add_non_blank(&mut errors, "session_id", session_id);
            }
            Self::ResponseCreate { session_id, messages, .. } => {
                add_non_blank(&mut errors, "session_id", session_id);
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
        messages: Vec<AgentThreadMessageResponse>,
    },
    #[serde(rename = "agent.error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_id: Option<String>,
    },
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
        Self {
            id: record.id,
            thread_id: record.thread_id,
            turn_index: record.turn_index,
            role: message.role,
            content,
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

#[cfg(test)]
mod tests {
    use slab_agent::port::ThreadMessageRecord;
    use slab_types::{
        ConversationMessage, ConversationMessageContent, ConversationToolCall,
        ConversationToolFunction,
    };

    use super::AgentThreadMessageResponse;

    #[test]
    fn agent_thread_message_response_hides_assistant_tool_calls() {
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
    }
}
