pub use slab_types::chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationMessage,
};

use crate::api::v1::chat::schema::{
    ChatCompletionRequest, ChatMessage, ChatReasoningEffort as ApiChatReasoningEffort,
    ChatThinkingConfig as ApiChatThinkingConfig, ChatThinkingType as ApiChatThinkingType,
    ChatVerbosity as ApiChatVerbosity,
};
use crate::infra::db;
use futures::stream::BoxStream;

pub enum ChatStreamChunk {
    Data(String),
    Comment(String),
}

pub enum ChatCompletionOutput {
    Json(ChatCompletionResult),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

#[derive(Debug, Clone)]
pub struct ChatModelOption {
    pub id: String,
    pub display_name: String,
    pub source: ChatModelSource,
    pub downloaded: bool,
    pub pending: bool,
    pub backend_id: Option<String>,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionCommand {
    pub id: Option<String>,
    pub model: String,
    pub messages: Vec<ConversationMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
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
        Self { role: message.role, content: message.content }
    }
}

impl From<db::ChatMessage> for ConversationMessage {
    fn from(message: db::ChatMessage) -> Self {
        Self { role: message.role, content: message.content }
    }
}

impl From<ChatCompletionRequest> for ChatCompletionCommand {
    fn from(request: ChatCompletionRequest) -> Self {
        let reasoning_effort = request
            .reasoning_effort
            .map(Into::into)
            .or_else(|| request.thinking.as_ref().and_then(reasoning_effort_from_thinking));
        let verbosity = request
            .verbosity
            .map(Into::into)
            .or_else(|| request.thinking.as_ref().and_then(verbosity_from_thinking));

        Self {
            id: request.id,
            model: request.model,
            messages: request.messages.into_iter().map(Into::into).collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            reasoning_effort,
            verbosity,
            stream: request.stream,
        }
    }
}

impl From<ApiChatReasoningEffort> for ChatReasoningEffort {
    fn from(value: ApiChatReasoningEffort) -> Self {
        match value {
            ApiChatReasoningEffort::None => Self::None,
            ApiChatReasoningEffort::Low => Self::Low,
            ApiChatReasoningEffort::Medium => Self::Medium,
            ApiChatReasoningEffort::High => Self::High,
            ApiChatReasoningEffort::Minimal => Self::Minimal,
        }
    }
}

impl From<ApiChatVerbosity> for ChatVerbosity {
    fn from(value: ApiChatVerbosity) -> Self {
        match value {
            ApiChatVerbosity::Low => Self::Low,
            ApiChatVerbosity::Medium => Self::Medium,
            ApiChatVerbosity::High => Self::High,
        }
    }
}

fn reasoning_effort_from_thinking(thinking: &ApiChatThinkingConfig) -> Option<ChatReasoningEffort> {
    match thinking.mode {
        ApiChatThinkingType::Disabled => Some(ChatReasoningEffort::None),
        // Default enabled thinking to `medium` so a plain toggle still has effect.
        ApiChatThinkingType::Enabled => {
            thinking.reasoning_effort.map(Into::into).or(Some(ChatReasoningEffort::Medium))
        }
    }
}

fn verbosity_from_thinking(thinking: &ApiChatThinkingConfig) -> Option<ChatVerbosity> {
    match thinking.mode {
        ApiChatThinkingType::Disabled => None,
        ApiChatThinkingType::Enabled => thinking.verbosity.map(Into::into),
    }
}

#[cfg(test)]
mod test {
    use super::{ChatCompletionCommand, ChatReasoningEffort, ChatVerbosity};
    use crate::api::v1::chat::schema::{
        ChatCompletionRequest, ChatMessage, ChatThinkingConfig, ChatThinkingType,
    };

    fn make_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            id: None,
            model: "cloud/provider/model".to_owned(),
            messages: vec![ChatMessage { role: "user".to_owned(), content: "hello".to_owned() }],
            stream: true,
            max_tokens: None,
            temperature: None,
            thinking: None,
            reasoning_effort: None,
            verbosity: None,
        }
    }

    #[test]
    fn thinking_disabled_maps_to_reasoning_none() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Disabled,
            reasoning_effort: None,
            verbosity: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::None)));
    }

    #[test]
    fn thinking_enabled_defaults_to_medium_reasoning() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Enabled,
            reasoning_effort: None,
            verbosity: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::Medium)));
    }

    #[test]
    fn explicit_reasoning_and_verbosity_take_precedence() {
        let mut request = make_request();
        request.thinking = Some(ChatThinkingConfig {
            mode: ChatThinkingType::Disabled,
            reasoning_effort: None,
            verbosity: None,
        });
        request.reasoning_effort = Some(crate::api::v1::chat::schema::ChatReasoningEffort::High);
        request.verbosity = Some(crate::api::v1::chat::schema::ChatVerbosity::Low);

        let command = ChatCompletionCommand::from(request);

        assert!(matches!(command.reasoning_effort, Some(ChatReasoningEffort::High)));
        assert!(matches!(command.verbosity, Some(ChatVerbosity::Low)));
    }
}
