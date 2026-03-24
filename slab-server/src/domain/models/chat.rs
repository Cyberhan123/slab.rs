pub use slab_types::chat::{
    ChatModelSource, ChatReasoningEffort, ChatVerbosity, ConversationContentPart,
    ConversationMessage, ConversationMessageContent, ConversationToolCall, ConversationToolFunction,
};
pub use slab_types::inference::TextGenerationUsage;

use crate::api::v1::chat::schema::{
    ChatCompletionRequest, ChatReasoningEffort as ApiChatReasoningEffort,
    ChatStreamOptions as ApiChatStreamOptions, ChatThinkingConfig as ApiChatThinkingConfig,
    ChatThinkingType as ApiChatThinkingType, ChatVerbosity as ApiChatVerbosity, CompletionRequest,
};
use crate::infra::db;
use futures::stream::BoxStream;

pub enum ChatStreamChunk {
    Data(String),
}

pub enum ChatCompletionOutput {
    Json(ChatCompletionResult),
    Stream(BoxStream<'static, ChatStreamChunk>),
}

pub enum TextCompletionOutput {
    Json(TextCompletionResult),
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
    pub top_p: Option<f32>,
    pub n: u32,
    pub stop: Vec<String>,
    pub grammar: Option<String>,
    pub grammar_json: bool,
    pub reasoning_effort: Option<ChatReasoningEffort>,
    pub verbosity: Option<ChatVerbosity>,
    pub stream: bool,
    pub stream_options: ChatStreamOptions,
}

#[derive(Debug, Clone)]
pub struct TextCompletionCommand {
    pub model: String,
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: u32,
    pub stop: Vec<String>,
    pub grammar: Option<String>,
    pub grammar_json: bool,
    pub stream: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ChatStreamOptions {
    pub include_usage: bool,
}

impl Default for ChatStreamOptions {
    fn default() -> Self {
        Self { include_usage: true }
    }
}

#[derive(Debug, Clone)]
pub struct ChatResultChoice {
    pub index: u32,
    pub message: ConversationMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<ChatResultChoice>,
    pub usage: Option<TextGenerationUsage>,
}

#[derive(Debug, Clone)]
pub struct TextResultChoice {
    pub index: u32,
    pub text: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TextCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub system_fingerprint: String,
    pub choices: Vec<TextResultChoice>,
    pub usage: Option<TextGenerationUsage>,
}

impl From<db::ChatMessage> for ConversationMessage {
    fn from(message: db::ChatMessage) -> Self {
        Self {
            role: message.role,
            content: ConversationMessageContent::Text(message.content),
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
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

impl From<ApiChatStreamOptions> for ChatStreamOptions {
    fn from(value: ApiChatStreamOptions) -> Self {
        Self { include_usage: value.include_usage }
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
        let grammar_json = request.grammar_json_requested();
        let stop = request.normalized_stop();

        Self {
            id: request.id,
            model: request.model.trim().to_owned(),
            messages: request.messages.into_iter().map(Into::into).collect(),
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            n: request.n.unwrap_or(1),
            stop,
            grammar: request.grammar,
            grammar_json,
            reasoning_effort,
            verbosity,
            stream: request.stream,
            stream_options: request.stream_options.map(Into::into).unwrap_or_default(),
        }
    }
}

fn reasoning_effort_from_thinking(thinking: &ApiChatThinkingConfig) -> Option<ChatReasoningEffort> {
    match thinking.mode {
        ApiChatThinkingType::Disabled => Some(ChatReasoningEffort::None),
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

impl From<CompletionRequest> for TextCompletionCommand {
    fn from(request: CompletionRequest) -> Self {
        let grammar_json = request.grammar_json_requested();
        let stop = request.normalized_stop();

        Self {
            model: request.model.trim().to_owned(),
            prompt: request.prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            n: request.n.unwrap_or(1),
            stop,
            grammar: request.grammar,
            grammar_json,
            stream: request.stream,
        }
    }
}

#[cfg(test)]
mod test {
    use super::{
        ChatCompletionCommand, ChatReasoningEffort, ChatVerbosity, TextCompletionCommand,
    };
    use crate::api::v1::chat::schema::{
        ChatCompletionRequest, ChatMessage, ChatResponseFormat, ChatResponseFormatType,
        ChatStreamOptions, ChatThinkingConfig, ChatThinkingType, CompletionRequest,
    };
    use serde_json::json;

    fn make_request() -> ChatCompletionRequest {
        ChatCompletionRequest {
            id: None,
            model: "cloud/provider/model".to_owned(),
            messages: vec![ChatMessage {
                role: "user".to_owned(),
                content: Some(crate::api::v1::chat::schema::ChatMessageContent::Text(
                    "hello".to_owned(),
                )),
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            stream: true,
            stream_options: Some(ChatStreamOptions { include_usage: true }),
            max_tokens: None,
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            grammar: None,
            response_format: None,
            json_schema: None,
            thinking: None,
            reasoning_effort: None,
            verbosity: None,
        }
    }

    fn make_completion_request() -> CompletionRequest {
        CompletionRequest {
            model: "cloud/provider/model".to_owned(),
            prompt: "hello".to_owned(),
            max_tokens: None,
            temperature: None,
            top_p: None,
            n: None,
            stop: None,
            stream: false,
            grammar: None,
            response_format: None,
            json_schema: None,
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

    #[test]
    fn response_format_json_object_maps_to_grammar_json() {
        let mut request = make_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: None,
            json_schema: None,
        });

        let command = ChatCompletionCommand::from(request);

        assert!(command.grammar_json);
    }

    #[test]
    fn json_schema_field_maps_to_grammar_json() {
        let mut request = make_request();
        request.json_schema = Some(json!({ "const": "42" }));

        let command = ChatCompletionCommand::from(request);

        assert!(command.grammar_json);
    }

    #[test]
    fn stop_string_normalizes_to_vec() {
        let mut request = make_request();
        request.stop = Some(crate::api::v1::chat::schema::StopSequences::Single(
            "END".to_owned(),
        ));

        let command = ChatCompletionCommand::from(request);

        assert_eq!(command.stop, vec!["END".to_owned()]);
    }

    #[test]
    fn completion_request_defaults_n_to_one() {
        let command = TextCompletionCommand::from(make_completion_request());

        assert_eq!(command.n, 1);
    }

    #[test]
    fn completion_response_format_maps_to_grammar_json() {
        let mut request = make_completion_request();
        request.response_format = Some(ChatResponseFormat {
            format_type: ChatResponseFormatType::JsonObject,
            schema: None,
            json_schema: None,
        });

        let command = TextCompletionCommand::from(request);

        assert!(command.grammar_json);
    }
}
