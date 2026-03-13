use genai::chat::ChatMessage as GenaiChatMessage;

use crate::api::v1::chat::schema::{ChatCompletionRequest, ChatMessage};
use crate::infra::db;

#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ChatModelSource {
    Local,
    Cloud,
}

#[derive(Debug, Clone)]
pub struct ChatModelOption {
    pub id: String,
    pub display_name: String,
    pub source: ChatModelSource,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
    pub backend_id: Option<String>,
    pub downloaded: bool,
    pub pending: bool,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionCommand {
    pub id: Option<String>,
    pub model: String,
    pub messages: Vec<ConversationMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stream: bool,
}

#[derive(Debug, Clone)]
pub struct ChatResultChoice {
    pub index: u32,
    pub message: ConversationMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatResultChoice>,
}

impl From<ChatMessage> for ConversationMessage {
    fn from(message: ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content,
        }
    }
}

impl From<db::ChatMessage> for ConversationMessage {
    fn from(message: db::ChatMessage) -> Self {
        Self {
            role: message.role,
            content: message.content,
        }
    }
}

impl From<&ConversationMessage> for GenaiChatMessage {
    fn from(message: &ConversationMessage) -> Self {
        match message.role.as_str() {
            "system" => Self::system(message.content.clone()),
            "assistant" => Self::assistant(message.content.clone()),
            _ => Self::user(message.content.clone()),
        }
    }
}

impl From<ChatCompletionRequest> for ChatCompletionCommand {
    fn from(request: ChatCompletionRequest) -> Self {
        Self {
            id: request.id,
            model: request.model,
            messages: request.messages.into_iter().map(Into::into).collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: request.stream,
        }
    }
}
