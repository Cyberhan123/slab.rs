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
