use crate::contexts::chat::domain::{ChatCompletionCommand, ChatCompletionResult, ConversationMessage};
use crate::schemas::v1::chat::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
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

pub fn to_chat_completion_response(result: ChatCompletionResult) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: result.id,
        object: result.object,
        created: result.created,
        model: result.model,
        choices: result
            .choices
            .into_iter()
            .map(|c| ChatChoice {
                index: c.index,
                message: OpenAiMessage {
                    role: c.message.role,
                    content: c.message.content,
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
    }
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
