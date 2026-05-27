use crate::models;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponsesClientEvent {
    /// The type of the client event. Always `response.create`.
    #[serde(rename = "type")]
    pub r#type: ResponseClientEventType,
    /// Set of 16 key-value pairs that can be attached to an object. This can be useful for storing additional information about the object in a structured format, and querying for objects via API or the dashboard.  Keys are strings with a maximum length of 64 characters. Values are strings with a maximum length of 512 characters.
    #[serde(rename = "metadata", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    /// An integer between 0 and 20 specifying the maximum number of most likely tokens to return at each token position, each with an associated log probability. In some cases, the number of returned tokens may be fewer than requested.
    #[serde(rename = "top_logprobs", skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i32>,
    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic. We generally recommend altering this or `top_p` but not both.
    #[serde(rename = "temperature", skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. So 0.1 means only the tokens comprising the top 10% probability mass are considered.  We generally recommend altering this or `temperature` but not both.
    #[serde(rename = "top_p", skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// This field is being replaced by `safety_identifier` and `prompt_cache_key`. Use `prompt_cache_key` instead to maintain caching optimizations. A stable identifier for your end-users. Used to boost cache hit rates by better bucketing similar requests and  to help OpenAI detect and prevent abuse. [Learn more](/docs/guides/safety-best-practices#safety-identifiers).
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// A stable identifier used to help detect users of your application that may be violating OpenAI's usage policies. The IDs should be a string that uniquely identifies each user, with a maximum length of 64 characters. We recommend hashing their username or email address, in order to avoid sending us any identifying information. [Learn more](/docs/guides/safety-best-practices#safety-identifiers).
    #[serde(rename = "safety_identifier", skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    /// Used by OpenAI to cache responses for similar requests to optimize your cache hit rates. Replaces the `user` field. [Learn more](/docs/guides/prompt-caching).
    #[serde(rename = "prompt_cache_key", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(
        rename = "service_tier",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<models::ServiceTier>>,
    /// The retention policy for the prompt cache. Set to `24h` to enable extended prompt caching, which keeps cached prefixes active for longer, up to a maximum of 24 hours. [Learn more](/docs/guides/prompt-caching#prompt-cache-retention).
    #[serde(rename = "prompt_cache_retention", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<PromptCacheRetention>,
    /// The unique ID of the previous response to the model. Use this to create multi-turn conversations. Learn more about [conversation state](/docs/guides/conversation-state). Cannot be used in conjunction with `conversation`.
    #[serde(rename = "previous_response_id", skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    /// Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI offers a wide range of models with different capabilities, performance characteristics, and price points. Refer to the [model guide](/docs/models) to browse and compare available models.
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::ModelIdsResponses>>,
    #[serde(rename = "reasoning", skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Box<models::Reasoning>>,
    /// Whether to run the model response in the background. [Learn more](/docs/guides/background).
    #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    /// The maximum number of total calls to built-in tools that can be processed in a response. This maximum number applies across all built-in tool calls, not per individual tool. Any further attempts to call a tool by the model will be ignored.
    #[serde(rename = "max_tool_calls", skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i32>,
    #[serde(rename = "text", skip_serializing_if = "Option::is_none")]
    pub text: Option<Box<models::ResponseTextParam>>,
    /// An array of tools the model may call while generating a response. You can specify which tool to use by setting the `tool_choice` parameter.  We support the following categories of tools: - **Built-in tools**: Tools that are provided by OpenAI that extend the   model's capabilities, like [web search](/docs/guides/tools-web-search)   or [file search](/docs/guides/tools-file-search). Learn more about   [built-in tools](/docs/guides/tools). - **MCP Tools**: Integrations with third-party systems via custom MCP servers   or predefined connectors such as Google Drive and SharePoint. Learn more about   [MCP Tools](/docs/guides/tools-connectors-mcp). - **Function calls (custom tools)**: Functions that are defined by you,   enabling the model to call your own code with strongly typed arguments   and outputs. Learn more about   [function calling](/docs/guides/function-calling). You can also use   custom tools to call your own code.
    #[serde(rename = "tools", skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<models::Tool>>,
    #[serde(rename = "tool_choice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Box<models::ToolChoiceParam>>,
    #[serde(
        rename = "prompt",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt: Option<Option<Box<models::Prompt>>>,
    /// The truncation strategy to use for the model response. - `auto`: If the input to this Response exceeds   the model's context window size, the model will truncate the   response to fit the context window by dropping items from the beginning of the conversation. - `disabled` (default): If the input size will exceed the context window   size for a model, the request will fail with a 400 error.
    #[serde(rename = "truncation", skip_serializing_if = "Option::is_none")]
    pub truncation: Option<Truncation>,
    #[serde(rename = "input", skip_serializing_if = "Option::is_none")]
    pub input: Option<Box<models::InputParam>>,
    /// Specify additional output data to include in the model response. Currently supported values are: - `web_search_call.action.sources`: Include the sources of the web search tool call. - `code_interpreter_call.outputs`: Includes the outputs of python code execution in code interpreter tool call items. - `computer_call_output.output.image_url`: Include image urls from the computer call output. - `file_search_call.results`: Include the search results of the file search tool call. - `message.input_image.image_url`: Include image urls from the input message. - `message.output_text.logprobs`: Include logprobs with assistant messages. - `reasoning.encrypted_content`: Includes an encrypted version of reasoning tokens in reasoning item outputs. This enables reasoning items to be used in multi-turn conversations when using the Responses API statelessly (like when the `store` parameter is set to `false`, or when an organization is enrolled in the zero data retention program).
    #[serde(rename = "include", skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<models::IncludeEnum>>,
    /// Whether to allow the model to run tool calls in parallel.
    #[serde(rename = "parallel_tool_calls", skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Whether to store the generated model response for later retrieval via API.
    #[serde(rename = "store", skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// A system (or developer) message inserted into the model's context.  When using along with `previous_response_id`, the instructions from a previous response will not be carried over to the next response. This makes it simple to swap out system (or developer) messages in new responses.
    #[serde(rename = "instructions", skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// If set to true, the model response data will be streamed to the client as it is generated using [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#Event_stream_format). See the [Streaming section below](/docs/api-reference/responses-streaming) for more information.
    #[serde(rename = "stream", skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(
        rename = "stream_options",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_options: Option<Option<Box<models::ResponseStreamOptions>>>,
    #[serde(rename = "conversation", skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Box<serde_json::Value>>,
    /// Context management configuration for this request.
    #[serde(rename = "context_management", skip_serializing_if = "Option::is_none")]
    pub context_management: Option<Vec<models::ContextManagementParam>>,
    /// An upper bound for the number of tokens that can be generated for a response, including visible output tokens and [reasoning tokens](/docs/guides/reasoning).
    #[serde(rename = "max_output_tokens", skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
}

impl ResponsesClientEvent {
    /// Client events accepted by the Responses WebSocket server.
    pub fn new(r#type: ResponseClientEventType) -> ResponsesClientEvent {
        ResponsesClientEvent {
            r#type,
            metadata: None,
            top_logprobs: None,
            temperature: None,
            top_p: None,
            user: None,
            safety_identifier: None,
            prompt_cache_key: None,
            service_tier: None,
            prompt_cache_retention: None,
            previous_response_id: None,
            model: None,
            reasoning: None,
            background: None,
            max_tool_calls: None,
            text: None,
            tools: None,
            tool_choice: None,
            prompt: None,
            truncation: None,
            input: None,
            include: None,
            parallel_tool_calls: None,
            store: None,
            instructions: None,
            stream: None,
            stream_options: None,
            conversation: None,
            context_management: None,
            max_output_tokens: None,
        }
    }
}
/// The type of the client event. Always `response.create`.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum ResponseClientEventType {
    #[serde(rename = "response.create")]
    #[default]
    ResponseCreate,
}

/// The retention policy for the prompt cache. Set to `24h` to enable extended prompt caching, which keeps cached prefixes active for longer, up to a maximum of 24 hours. [Learn more](/docs/guides/prompt-caching#prompt-cache-retention).
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum PromptCacheRetention {
    #[serde(rename = "in_memory")]
    #[default]
    InMemory,
    #[serde(rename = "24h")]
    Variant24h,
}

/// The truncation strategy to use for the model response. - `auto`: If the input to this Response exceeds   the model's context window size, the model will truncate the   response to fit the context window by dropping items from the beginning of the conversation. - `disabled` (default): If the input size will exceed the context window   size for a model, the request will fail with a 400 error.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum Truncation {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponsesClientEventResponseCreate {
    /// The type of the client event. Always `response.create`.
    #[serde(rename = "type")]
    pub r#type: ResponseCreateEventType,
    /// Set of 16 key-value pairs that can be attached to an object. This can be useful for storing additional information about the object in a structured format, and querying for objects via API or the dashboard.  Keys are strings with a maximum length of 64 characters. Values are strings with a maximum length of 512 characters.
    #[serde(rename = "metadata", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, String>>,
    /// An integer between 0 and 20 specifying the maximum number of most likely tokens to return at each token position, each with an associated log probability. In some cases, the number of returned tokens may be fewer than requested.
    #[serde(rename = "top_logprobs", skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<i32>,
    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic. We generally recommend altering this or `top_p` but not both.
    #[serde(rename = "temperature", skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. So 0.1 means only the tokens comprising the top 10% probability mass are considered.  We generally recommend altering this or `temperature` but not both.
    #[serde(rename = "top_p", skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// This field is being replaced by `safety_identifier` and `prompt_cache_key`. Use `prompt_cache_key` instead to maintain caching optimizations. A stable identifier for your end-users. Used to boost cache hit rates by better bucketing similar requests and  to help OpenAI detect and prevent abuse. [Learn more](/docs/guides/safety-best-practices#safety-identifiers).
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// A stable identifier used to help detect users of your application that may be violating OpenAI's usage policies. The IDs should be a string that uniquely identifies each user, with a maximum length of 64 characters. We recommend hashing their username or email address, in order to avoid sending us any identifying information. [Learn more](/docs/guides/safety-best-practices#safety-identifiers).
    #[serde(rename = "safety_identifier", skip_serializing_if = "Option::is_none")]
    pub safety_identifier: Option<String>,
    /// Used by OpenAI to cache responses for similar requests to optimize your cache hit rates. Replaces the `user` field. [Learn more](/docs/guides/prompt-caching).
    #[serde(rename = "prompt_cache_key", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
    #[serde(
        rename = "service_tier",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<models::ServiceTier>>,
    /// The retention policy for the prompt cache. Set to `24h` to enable extended prompt caching, which keeps cached prefixes active for longer, up to a maximum of 24 hours. [Learn more](/docs/guides/prompt-caching#prompt-cache-retention).
    #[serde(rename = "prompt_cache_retention", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_retention: Option<ResponseCreatePromptCacheRetention>,
    /// The unique ID of the previous response to the model. Use this to create multi-turn conversations. Learn more about [conversation state](/docs/guides/conversation-state). Cannot be used in conjunction with `conversation`.
    #[serde(rename = "previous_response_id", skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
    /// Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI offers a wide range of models with different capabilities, performance characteristics, and price points. Refer to the [model guide](/docs/models) to browse and compare available models.
    #[serde(rename = "model", skip_serializing_if = "Option::is_none")]
    pub model: Option<Box<models::ModelIdsResponses>>,
    #[serde(rename = "reasoning", skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Box<models::Reasoning>>,
    /// Whether to run the model response in the background. [Learn more](/docs/guides/background).
    #[serde(rename = "background", skip_serializing_if = "Option::is_none")]
    pub background: Option<bool>,
    /// The maximum number of total calls to built-in tools that can be processed in a response. This maximum number applies across all built-in tool calls, not per individual tool. Any further attempts to call a tool by the model will be ignored.
    #[serde(rename = "max_tool_calls", skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<i32>,
    #[serde(rename = "text", skip_serializing_if = "Option::is_none")]
    pub text: Option<Box<models::ResponseTextParam>>,
    /// An array of tools the model may call while generating a response. You can specify which tool to use by setting the `tool_choice` parameter.  We support the following categories of tools: - **Built-in tools**: Tools that are provided by OpenAI that extend the   model's capabilities, like [web search](/docs/guides/tools-web-search)   or [file search](/docs/guides/tools-file-search). Learn more about   [built-in tools](/docs/guides/tools). - **MCP Tools**: Integrations with third-party systems via custom MCP servers   or predefined connectors such as Google Drive and SharePoint. Learn more about   [MCP Tools](/docs/guides/tools-connectors-mcp). - **Function calls (custom tools)**: Functions that are defined by you,   enabling the model to call your own code with strongly typed arguments   and outputs. Learn more about   [function calling](/docs/guides/function-calling). You can also use   custom tools to call your own code.
    #[serde(rename = "tools", skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<models::Tool>>,
    #[serde(rename = "tool_choice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Box<models::ToolChoiceParam>>,
    #[serde(
        rename = "prompt",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub prompt: Option<Option<Box<models::Prompt>>>,
    /// The truncation strategy to use for the model response. - `auto`: If the input to this Response exceeds   the model's context window size, the model will truncate the   response to fit the context window by dropping items from the beginning of the conversation. - `disabled` (default): If the input size will exceed the context window   size for a model, the request will fail with a 400 error.
    #[serde(rename = "truncation", skip_serializing_if = "Option::is_none")]
    pub truncation: Option<ResponseCreateTruncation>,
    #[serde(rename = "input", skip_serializing_if = "Option::is_none")]
    pub input: Option<Box<models::InputParam>>,
    /// Specify additional output data to include in the model response. Currently supported values are: - `web_search_call.action.sources`: Include the sources of the web search tool call. - `code_interpreter_call.outputs`: Includes the outputs of python code execution in code interpreter tool call items. - `computer_call_output.output.image_url`: Include image urls from the computer call output. - `file_search_call.results`: Include the search results of the file search tool call. - `message.input_image.image_url`: Include image urls from the input message. - `message.output_text.logprobs`: Include logprobs with assistant messages. - `reasoning.encrypted_content`: Includes an encrypted version of reasoning tokens in reasoning item outputs. This enables reasoning items to be used in multi-turn conversations when using the Responses API statelessly (like when the `store` parameter is set to `false`, or when an organization is enrolled in the zero data retention program).
    #[serde(rename = "include", skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<models::IncludeEnum>>,
    /// Whether to allow the model to run tool calls in parallel.
    #[serde(rename = "parallel_tool_calls", skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    /// Whether to store the generated model response for later retrieval via API.
    #[serde(rename = "store", skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// A system (or developer) message inserted into the model's context.  When using along with `previous_response_id`, the instructions from a previous response will not be carried over to the next response. This makes it simple to swap out system (or developer) messages in new responses.
    #[serde(rename = "instructions", skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// If set to true, the model response data will be streamed to the client as it is generated using [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#Event_stream_format). See the [Streaming section below](/docs/api-reference/responses-streaming) for more information.
    #[serde(rename = "stream", skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(
        rename = "stream_options",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_options: Option<Option<Box<models::ResponseStreamOptions>>>,
    #[serde(rename = "conversation", skip_serializing_if = "Option::is_none")]
    pub conversation: Option<Box<serde_json::Value>>,
    /// Context management configuration for this request.
    #[serde(rename = "context_management", skip_serializing_if = "Option::is_none")]
    pub context_management: Option<Vec<models::ContextManagementParam>>,
    /// An upper bound for the number of tokens that can be generated for a response, including visible output tokens and [reasoning tokens](/docs/guides/reasoning).
    #[serde(rename = "max_output_tokens", skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
}

impl ResponsesClientEventResponseCreate {
    /// Client event for creating a response over a persistent WebSocket connection. This payload uses the same top-level fields as `POST /v1/responses`.  Notes: - `stream` is implicit over WebSocket and should not be sent. - `background` is not supported over WebSocket.
    pub fn new(r#type: ResponseCreateEventType) -> ResponsesClientEventResponseCreate {
        ResponsesClientEventResponseCreate {
            r#type,
            metadata: None,
            top_logprobs: None,
            temperature: None,
            top_p: None,
            user: None,
            safety_identifier: None,
            prompt_cache_key: None,
            service_tier: None,
            prompt_cache_retention: None,
            previous_response_id: None,
            model: None,
            reasoning: None,
            background: None,
            max_tool_calls: None,
            text: None,
            tools: None,
            tool_choice: None,
            prompt: None,
            truncation: None,
            input: None,
            include: None,
            parallel_tool_calls: None,
            store: None,
            instructions: None,
            stream: None,
            stream_options: None,
            conversation: None,
            context_management: None,
            max_output_tokens: None,
        }
    }
}
/// The type of the client event. Always `response.create`.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum ResponseCreateEventType {
    #[serde(rename = "response.create")]
    #[default]
    ResponseCreate,
}

/// The retention policy for the prompt cache. Set to `24h` to enable extended prompt caching, which keeps cached prefixes active for longer, up to a maximum of 24 hours. [Learn more](/docs/guides/prompt-caching#prompt-cache-retention).
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum ResponseCreatePromptCacheRetention {
    #[serde(rename = "in_memory")]
    #[default]
    InMemory,
    #[serde(rename = "24h")]
    Variant24h,
}

/// The truncation strategy to use for the model response. - `auto`: If the input to this Response exceeds   the model's context window size, the model will truncate the   response to fit the context window by dropping items from the beginning of the conversation. - `disabled` (default): If the input size will exceed the context window   size for a model, the request will fail with a 400 error.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum ResponseCreateTruncation {
    #[serde(rename = "auto")]
    #[default]
    Auto,
    #[serde(rename = "disabled")]
    Disabled,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResponsesServerEvent {
    /// The event type identifier.
    #[serde(rename = "type")]
    pub r#type: ServerEventType,
    /// The sequence number of this event.
    #[serde(rename = "sequence_number")]
    pub sequence_number: i32,
    /// The incremental input data (delta) for the custom tool call.
    #[serde(rename = "delta")]
    pub delta: String,
    /// The index of the output this event applies to.
    #[serde(rename = "output_index")]
    pub output_index: i32,
    /// Unique identifier for the API item associated with this event.
    #[serde(rename = "item_id")]
    pub item_id: String,
    /// The error code.
    #[serde(rename = "code")]
    pub code: String,
    /// The full response object that is queued.
    #[serde(rename = "response")]
    pub response: Box<models::Response>,
    /// The index of the content part within the output item.
    #[serde(rename = "content_index")]
    pub content_index: i32,
    #[serde(rename = "part")]
    pub part: Box<models::ResponseReasoningSummaryPartDoneEventPart>,
    /// The error message.
    #[serde(rename = "message")]
    pub message: String,
    /// The error parameter.
    #[serde(rename = "param")]
    pub param: String,
    /// The name of the function that was called.
    #[serde(rename = "name")]
    pub name: String,
    /// A JSON string containing the finalized arguments for the MCP tool call.
    #[serde(rename = "arguments")]
    pub arguments: String,
    /// The output item that was marked done.
    #[serde(rename = "item")]
    pub item: Box<models::OutputItem>,
    /// The index of the summary part within the reasoning summary.
    #[serde(rename = "summary_index")]
    pub summary_index: i32,
    /// The text content that is finalized.
    #[serde(rename = "text")]
    pub text: String,
    /// The refusal text that is finalized.
    #[serde(rename = "refusal")]
    pub refusal: String,
    /// The log probabilities of the tokens in the delta.
    #[serde(rename = "logprobs")]
    pub logprobs: Vec<models::ResponseLogProb>,
    /// 0-based index for the partial image (backend is 1-based, but this is 0-based for the user).
    #[serde(rename = "partial_image_index")]
    pub partial_image_index: i32,
    /// Base64-encoded partial image data, suitable for rendering as an image.
    #[serde(rename = "partial_image_b64")]
    pub partial_image_b64: String,
    /// The index of the annotation within the content part.
    #[serde(rename = "annotation_index")]
    pub annotation_index: i32,
    /// The annotation object being added. (See annotation schema for details.)
    #[serde(rename = "annotation")]
    pub annotation: serde_json::Value,
    /// The complete input data for the custom tool call.
    #[serde(rename = "input")]
    pub input: String,
}

impl ResponsesServerEvent {
    /// Server events emitted by the Responses WebSocket server.
    pub fn new(
        r#type: ServerEventType,
        sequence_number: i32,
        delta: String,
        output_index: i32,
        item_id: String,
        code: String,
        response: models::Response,
        content_index: i32,
        part: models::ResponseReasoningSummaryPartDoneEventPart,
        message: String,
        param: String,
        name: String,
        arguments: String,
        item: models::OutputItem,
        summary_index: i32,
        text: String,
        refusal: String,
        logprobs: Vec<models::ResponseLogProb>,
        partial_image_index: i32,
        partial_image_b64: String,
        annotation_index: i32,
        annotation: serde_json::Value,
        input: String,
    ) -> ResponsesServerEvent {
        ResponsesServerEvent {
            r#type,
            sequence_number,
            delta,
            output_index,
            item_id,
            code,
            response: Box::new(response),
            content_index,
            part: Box::new(part),
            message,
            param,
            name,
            arguments,
            item: Box::new(item),
            summary_index,
            text,
            refusal,
            logprobs,
            partial_image_index,
            partial_image_b64,
            annotation_index,
            annotation,
            input,
        }
    }
}
/// The event type identifier.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[derive(Default)]
pub enum ServerEventType {
    #[serde(rename = "response.custom_tool_call_input.done")]
    #[default]
    ResponseCustomToolCallInputDone,
}
