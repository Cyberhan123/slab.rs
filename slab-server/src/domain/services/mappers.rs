use crate::domain::models::{
    ChatCompletionCommand, ChatCompletionResult, ConversationMessage, ModelLoadCommand, ModelStatus,
    TaskResult,
};
use crate::infra::db::TaskRecord;
use crate::api::dto::v1::chat::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
};
use crate::api::dto::v1::models::{LoadModelRequest, ModelStatusResponse};
use crate::api::dto::v1::task::{TaskResponse, TaskResultPayload};

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
            .map(|choice| ChatChoice {
                index: choice.index,
                message: OpenAiMessage {
                    role: choice.message.role,
                    content: choice.message.content,
                },
                finish_reason: choice.finish_reason,
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

pub fn to_model_load_command(request: LoadModelRequest) -> ModelLoadCommand {
    ModelLoadCommand {
        backend_id: request.backend_id,
        model_path: request.model_path,
        num_workers: request.num_workers,
    }
}

pub fn to_model_status_response(status: ModelStatus) -> ModelStatusResponse {
    ModelStatusResponse {
        backend: status.backend,
        status: status.status,
    }
}

pub fn to_task_response(record: &TaskRecord) -> TaskResponse {
    TaskResponse {
        id: record.id.clone(),
        task_type: record.task_type.clone(),
        status: record.status.clone(),
        error_msg: record.error_msg.clone(),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

pub fn to_task_result_response(result: TaskResult) -> TaskResultPayload {
    TaskResultPayload {
        image: result.image,
        images: result.images,
        video_path: result.video_path,
        text: result.text,
    }
}
