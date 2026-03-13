use crate::contexts::chat::domain::{ChatCompletionCommand, ConversationMessage};
use crate::schemas::v1::chat::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
};

pub fn to_chat_completion_command(request: ChatCompletionRequest) -> ChatCompletionCommand {
    ChatCompletionCommand {
        id: request.id,
        model: request.model,
        messages: request
            .messages
            .into_iter()
            .map(|message| ConversationMessage {
                role: message.role,
                content: message.content,
            })
            .collect(),
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        stream: request.stream,
    }
}

pub fn to_chat_completion_response(response: ChatCompletionResponse) -> ChatCompletionResponse {
    response
}

pub fn to_openai_messages(messages: Vec<ConversationMessage>) -> Vec<OpenAiMessage> {
    messages
        .into_iter()
        .map(|message| OpenAiMessage {
            role: message.role,
            content: message.content,
        })
        .collect()
}
