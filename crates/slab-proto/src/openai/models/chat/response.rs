use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatCompletionResponse {
    /// A unique identifier for the chat completion.
    #[serde(rename = "id")]
    pub id: String,
    /// A list of chat completion choices. Can be more than one if `n` is greater than 1.
    #[serde(rename = "choices")]
    pub choices: Vec<models::CreateChatCompletionResponseChoicesInner>,
    /// The Unix timestamp (in seconds) of when the chat completion was created.
    #[serde(rename = "created")]
    pub created: i32,
    /// The model used for the chat completion.
    #[serde(rename = "model")]
    pub model: String,
    /// The object type, which is always `chat.completion`.
    #[serde(rename = "object")]
    pub object: ChatResponseObject,
    #[serde(
        rename = "service_tier",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<models::ServiceTier>>,
    /// This fingerprint represents the backend configuration that the model runs with.  Can be used in conjunction with the `seed` request parameter to understand when backend changes have been made that might impact determinism.
    #[serde(rename = "system_fingerprint", skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(rename = "usage", skip_serializing_if = "Option::is_none")]
    pub usage: Option<Box<models::CompletionUsage>>,
}

impl CreateChatCompletionResponse {
    /// Represents a chat completion response returned by model, based on the provided input.
    pub fn new(
        id: String,
        choices: Vec<models::CreateChatCompletionResponseChoicesInner>,
        created: i32,
        model: String,
        object: ChatResponseObject,
    ) -> CreateChatCompletionResponse {
        CreateChatCompletionResponse {
            id,
            choices,
            created,
            model,
            object,
            service_tier: None,
            system_fingerprint: None,
            usage: None,
        }
    }
}
/// The object type, which is always `chat.completion`.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum ChatResponseObject {
    #[serde(rename = "chat.completion")]
    #[default]
    ChatCompletion,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatCompletionResponseChoicesInner {
    /// The reason the model stopped generating tokens. This will be `stop` if the model hit a natural stop point or a provided stop sequence, `length` if the maximum number of tokens specified in the request was reached, `content_filter` if content was omitted due to a flag from our content filters, `tool_calls` if the model called a tool, or `function_call` (deprecated) if the model called a function.
    #[serde(rename = "finish_reason")]
    pub finish_reason: FinishReason,
    /// The index of the choice in the list of choices.
    #[serde(rename = "index")]
    pub index: i32,
    #[serde(rename = "message")]
    pub message: Box<models::ChatCompletionResponseMessage>,
    #[serde(rename = "logprobs", deserialize_with = "Option::deserialize")]
    pub logprobs: Option<Box<models::CreateChatCompletionResponseChoicesInnerLogprobs>>,
}

impl CreateChatCompletionResponseChoicesInner {
    pub fn new(
        finish_reason: FinishReason,
        index: i32,
        message: models::ChatCompletionResponseMessage,
        logprobs: Option<models::CreateChatCompletionResponseChoicesInnerLogprobs>,
    ) -> CreateChatCompletionResponseChoicesInner {
        CreateChatCompletionResponseChoicesInner {
            finish_reason,
            index,
            message: Box::new(message),
            logprobs: logprobs.map(Box::new),
        }
    }
}
/// The reason the model stopped generating tokens. This will be `stop` if the model hit a natural stop point or a provided stop sequence, `length` if the maximum number of tokens specified in the request was reached, `content_filter` if content was omitted due to a flag from our content filters, `tool_calls` if the model called a tool, or `function_call` (deprecated) if the model called a function.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Default,
)]
pub enum FinishReason {
    #[serde(rename = "stop")]
    #[default]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "tool_calls")]
    ToolCalls,
    #[serde(rename = "content_filter")]
    ContentFilter,
    #[serde(rename = "function_call")]
    FunctionCall,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatCompletionResponseChoicesInnerLogprobs {
    /// A list of message content tokens with log probability information.
    #[serde(rename = "content", deserialize_with = "Option::deserialize")]
    pub content: Option<Vec<models::ChatCompletionTokenLogprob>>,
    /// A list of message refusal tokens with log probability information.
    #[serde(rename = "refusal", deserialize_with = "Option::deserialize")]
    pub refusal: Option<Vec<models::ChatCompletionTokenLogprob>>,
}

impl CreateChatCompletionResponseChoicesInnerLogprobs {
    /// Log probability information for the choice.
    pub fn new(
        content: Option<Vec<models::ChatCompletionTokenLogprob>>,
        refusal: Option<Vec<models::ChatCompletionTokenLogprob>>,
    ) -> CreateChatCompletionResponseChoicesInnerLogprobs {
        CreateChatCompletionResponseChoicesInnerLogprobs { content, refusal }
    }
}
