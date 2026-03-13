#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
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

/// A single choice in a chat completion result.
#[derive(Debug, Clone)]
pub struct ChatResultChoice {
    pub index: u32,
    pub message: ConversationMessage,
    pub finish_reason: String,
}

/// Domain representation of a complete (non-streaming) chat completion.
///
/// This type is free of HTTP/schema annotations and is converted to
/// `ChatCompletionResponse` by the HTTP mapper at the transport boundary.
#[derive(Debug, Clone)]
pub struct ChatCompletionResult {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatResultChoice>,
}
