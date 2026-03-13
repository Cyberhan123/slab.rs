use crate::api::v1::chat::schema::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ChatMessage as OpenAiMessage,
};
use crate::api::v1::tasks::schema::{TaskResponse, TaskResultPayload};
use crate::domain::models::{
    AcceptedOperation, ChatCompletionCommand, ChatCompletionResult,
    ChatModelOption as DomainChatModelOption, ChatModelSource as DomainChatModelSource,
    ConversationMessage, TaskResult, TaskView,
};
use crate::infra::db::TaskRecord;

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

pub fn to_chat_model_option_response(
    option: DomainChatModelOption,
) -> crate::api::v1::chat::schema::ChatModelOption {
    crate::api::v1::chat::schema::ChatModelOption {
        id: option.id,
        display_name: option.display_name,
        source: match option.source {
            DomainChatModelSource::Local => crate::api::v1::chat::schema::ChatModelSource::Local,
            DomainChatModelSource::Cloud => crate::api::v1::chat::schema::ChatModelSource::Cloud,
        },
        provider_id: option.provider_id,
        provider_name: option.provider_name,
        backend_id: option.backend_id,
        downloaded: option.downloaded,
        pending: option.pending,
    }
}

pub fn to_task_view(record: &TaskRecord) -> TaskView {
    TaskView {
        id: record.id.clone(),
        task_type: record.task_type.clone(),
        status: record.status.clone(),
        error_msg: record.error_msg.clone(),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    }
}

pub fn to_task_response(view: TaskView) -> TaskResponse {
    TaskResponse {
        id: view.id,
        task_type: view.task_type,
        status: view.status,
        error_msg: view.error_msg,
        created_at: view.created_at,
        updated_at: view.updated_at,
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

pub fn to_operation_accepted_response(
    result: AcceptedOperation,
) -> crate::api::v1::tasks::schema::OperationAcceptedResponse {
    crate::api::v1::tasks::schema::OperationAcceptedResponse {
        operation_id: result.operation_id,
    }
}
