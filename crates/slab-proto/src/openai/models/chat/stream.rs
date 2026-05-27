pub mod create_chat_completion_stream_response {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateChatCompletionStreamResponse {
        /// A unique identifier for the chat completion. Each chunk has the same ID.
        #[serde(rename = "id")]
        pub id: String,
        /// A list of chat completion choices. Can contain more than one elements if `n` is greater than 1. Can also be empty for the last chunk if you set `stream_options: {\"include_usage\": true}`.
        #[serde(rename = "choices")]
        pub choices: Vec<models::CreateChatCompletionStreamResponseChoicesInner>,
        /// The Unix timestamp (in seconds) of when the chat completion was created. Each chunk has the same timestamp.
        #[serde(rename = "created")]
        pub created: i32,
        /// The model to generate the completion.
        #[serde(rename = "model")]
        pub model: String,
        /// The object type, which is always `chat.completion.chunk`.
        #[serde(rename = "object")]
        pub object: Object,
        #[serde(
            rename = "service_tier",
            default,
            with = "::serde_with::rust::double_option",
            skip_serializing_if = "Option::is_none"
        )]
        pub service_tier: Option<Option<models::ServiceTier>>,
        /// This fingerprint represents the backend configuration that the model runs with. Can be used in conjunction with the `seed` request parameter to understand when backend changes have been made that might impact determinism.
        #[serde(rename = "system_fingerprint", skip_serializing_if = "Option::is_none")]
        pub system_fingerprint: Option<String>,
        /// An optional field that will only be present when you set `stream_options: {\"include_usage\": true}` in your request. When present, it contains a null value **except for the last chunk** which contains the token usage statistics for the entire request.  **NOTE:** If the stream is interrupted or cancelled, you may not receive the final usage chunk which contains the total token usage for the request.
        #[serde(rename = "usage", skip_serializing_if = "Option::is_none")]
        pub usage: Option<Box<models::CompletionUsage>>,
    }

    impl CreateChatCompletionStreamResponse {
        /// Represents a streamed chunk of a chat completion response returned by the model, based on the provided input.  [Learn more](/docs/guides/streaming-responses).
        pub fn new(
            id: String,
            choices: Vec<models::CreateChatCompletionStreamResponseChoicesInner>,
            created: i32,
            model: String,
            object: Object,
        ) -> CreateChatCompletionStreamResponse {
            CreateChatCompletionStreamResponse {
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
    /// The object type, which is always `chat.completion.chunk`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum Object {
        #[serde(rename = "chat.completion.chunk")]
        ChatCompletionChunk,
    }

    impl Default for Object {
        fn default() -> Object {
            Self::ChatCompletionChunk
        }
    }
}

pub mod create_chat_completion_stream_response_choices_inner {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateChatCompletionStreamResponseChoicesInner {
        #[serde(rename = "delta")]
        pub delta: Box<models::ChatCompletionStreamResponseDelta>,
        /// The reason the model stopped generating tokens. This will be `stop` if the model hit a natural stop point or a provided stop sequence, `length` if the maximum number of tokens specified in the request was reached, `content_filter` if content was omitted due to a flag from our content filters, `tool_calls` if the model called a tool, or `function_call` (deprecated) if the model called a function.
        #[serde(rename = "finish_reason")]
        pub finish_reason: FinishReason,
        /// The index of the choice in the list of choices.
        #[serde(rename = "index")]
        pub index: i32,
        #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
        pub logprobs: Option<Box<models::CreateChatCompletionStreamResponseChoicesInnerLogprobs>>,
    }

    impl CreateChatCompletionStreamResponseChoicesInner {
        pub fn new(
            delta: models::ChatCompletionStreamResponseDelta,
            finish_reason: FinishReason,
            index: i32,
        ) -> CreateChatCompletionStreamResponseChoicesInner {
            CreateChatCompletionStreamResponseChoicesInner {
                delta: Box::new(delta),
                finish_reason,
                index,
                logprobs: None,
            }
        }
    }
    /// The reason the model stopped generating tokens. This will be `stop` if the model hit a natural stop point or a provided stop sequence, `length` if the maximum number of tokens specified in the request was reached, `content_filter` if content was omitted due to a flag from our content filters, `tool_calls` if the model called a tool, or `function_call` (deprecated) if the model called a function.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
    pub(crate) enum FinishReason {
        #[serde(rename = "stop")]
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

    impl Default for FinishReason {
        fn default() -> FinishReason {
            Self::Stop
        }
    }
}

pub mod create_chat_completion_stream_response_choices_inner_logprobs {
    use crate::models;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
    pub struct CreateChatCompletionStreamResponseChoicesInnerLogprobs {
        /// A list of message content tokens with log probability information.
        #[serde(rename = "content")]
        pub content: Vec<models::ChatCompletionTokenLogprob>,
        /// A list of message refusal tokens with log probability information.
        #[serde(rename = "refusal")]
        pub refusal: Vec<models::ChatCompletionTokenLogprob>,
    }

    impl CreateChatCompletionStreamResponseChoicesInnerLogprobs {
        /// Log probability information for the choice.
        pub fn new(
            content: Vec<models::ChatCompletionTokenLogprob>,
            refusal: Vec<models::ChatCompletionTokenLogprob>,
        ) -> CreateChatCompletionStreamResponseChoicesInnerLogprobs {
            CreateChatCompletionStreamResponseChoicesInnerLogprobs { content, refusal }
        }
    }
}

pub use create_chat_completion_stream_response::CreateChatCompletionStreamResponse;
pub(crate) use create_chat_completion_stream_response::Object;
pub use create_chat_completion_stream_response_choices_inner::CreateChatCompletionStreamResponseChoicesInner;
pub(crate) use create_chat_completion_stream_response_choices_inner::FinishReason;
pub use create_chat_completion_stream_response_choices_inner_logprobs::CreateChatCompletionStreamResponseChoicesInnerLogprobs;
