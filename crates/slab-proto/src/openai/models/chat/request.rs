use crate::openai::models;
use serde::{Deserialize, Serialize};

use super::meta::ChatMetaFormat;
use super::meta::ChatMetaPromptCacheRetention;
use super::meta::Modalities;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatCompletionRequest {
    /// A list of messages comprising the conversation so far. Depending on the [model](/docs/models) you use, different message types (modalities) are supported, like [text](/docs/guides/text-generation), [images](/docs/guides/vision), and [audio](/docs/guides/audio).
    #[serde(rename = "messages")]
    pub messages: Vec<models::ChatCompletionRequestMessage>,
    /// Model ID used to generate the response, like `gpt-4o` or `o3`. OpenAI offers a wide range of models with different capabilities, performance characteristics, and price points. Refer to the [model guide](/docs/models) to browse and compare available models.
    #[serde(rename = "model")]
    pub model: Box<models::ModelIdsShared>,
    /// Set of 16 key-value pairs that can be attached to an object. This can be useful for storing additional information about the object in a structured format, and querying for objects via API or the dashboard.  Keys are strings with a maximum length of 64 characters. Values are strings with a maximum length of 512 characters.
    #[serde(
        rename = "metadata",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<Option<std::collections::HashMap<String, String>>>,
    /// An integer between 0 and 20 specifying the maximum number of most likely tokens to return at each token position, each with an associated log probability. In some cases, the number of returned tokens may be fewer than requested. `logprobs` must be set to `true` if this parameter is used.
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
    pub prompt_cache_retention: Option<ChatMetaPromptCacheRetention>,
    /// Output types that you would like the model to generate. Most models are capable of generating text, which is the default:  `[\"text\"]`  The `gpt-4o-audio-preview` model can also be used to [generate audio](/docs/guides/audio). To request that this model generate both text and audio responses, you can use:  `[\"text\", \"audio\"]`
    #[serde(
        rename = "modalities",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub modalities: Option<Option<Vec<Modalities>>>,
    #[serde(
        rename = "verbosity",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub verbosity: Option<Option<models::Verbosity>>,
    #[serde(
        rename = "reasoning_effort",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub reasoning_effort: Option<Option<models::ReasoningEffort>>,
    /// An upper bound for the number of tokens that can be generated for a completion, including visible output tokens and [reasoning tokens](/docs/guides/reasoning).
    #[serde(rename = "max_completion_tokens", skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i32>,
    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on their existing frequency in the text so far, decreasing the model's likelihood to repeat the same line verbatim.
    #[serde(rename = "frequency_penalty", skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on whether they appear in the text so far, increasing the model's likelihood to talk about new topics.
    #[serde(rename = "presence_penalty", skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(rename = "web_search_options", skip_serializing_if = "Option::is_none")]
    pub web_search_options: Option<Box<models::WebSearch>>,
    #[serde(rename = "response_format", skip_serializing_if = "Option::is_none")]
    pub response_format: Option<Box<models::CreateChatCompletionRequestAllOfResponseFormat>>,
    #[serde(rename = "audio", skip_serializing_if = "Option::is_none")]
    pub audio: Option<Box<models::CreateChatCompletionRequestAllOfAudio>>,
    /// Whether or not to store the output of this chat completion request for use in our [model distillation](/docs/guides/distillation) or [evals](/docs/guides/evals) products.  Supports text and image inputs. Note: image inputs over 8MB will be dropped.
    #[serde(rename = "store", skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// If set to true, the model response data will be streamed to the client as it is generated using [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#Event_stream_format). See the [Streaming section below](/docs/api-reference/chat/streaming) for more information, along with the [streaming responses](/docs/guides/streaming-responses) guide for more information on how to handle the streaming events.
    #[serde(rename = "stream", skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(rename = "stop", skip_serializing_if = "Option::is_none")]
    pub stop: Option<Box<models::StopConfiguration>>,
    /// Modify the likelihood of specified tokens appearing in the completion.  Accepts a JSON object that maps tokens (specified by their token ID in the tokenizer) to an associated bias value from -100 to 100. Mathematically, the bias is added to the logits generated by the model prior to sampling. The exact effect will vary per model, but values between -1 and 1 should decrease or increase likelihood of selection; values like -100 or 100 should result in a ban or exclusive selection of the relevant token.
    #[serde(rename = "logit_bias", skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::HashMap<String, i32>>,
    /// Whether to return log probabilities of the output tokens or not. If true, returns the log probabilities of each output token returned in the `content` of `message`.
    #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    /// The maximum number of [tokens](/tokenizer) that can be generated in the chat completion. This value can be used to control [costs](https://openai.com/api/pricing/) for text generated via API.  This value is now deprecated in favor of `max_completion_tokens`, and is not compatible with [o-series models](/docs/guides/reasoning).
    #[serde(rename = "max_tokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// How many chat completion choices to generate for each input message. Note that you will be charged based on the number of generated tokens across all of the choices. Keep `n` as `1` to minimize costs.
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    /// Configuration for a [Predicted Output](/docs/guides/predicted-outputs), which can greatly improve response times when large parts of the model response are known ahead of time. This is most common when you are regenerating a file with only minor changes to most of the content.
    #[serde(rename = "prediction", skip_serializing_if = "Option::is_none")]
    pub prediction: Option<Box<models::PredictionContent>>,
    /// This feature is in Beta. If specified, our system will make a best effort to sample deterministically, such that repeated requests with the same `seed` and parameters should return the same result. Determinism is not guaranteed, and you should refer to the `system_fingerprint` response parameter to monitor changes in the backend.
    #[serde(rename = "seed", skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    #[serde(
        rename = "stream_options",
        default,
        with = "::serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub stream_options: Option<Option<Box<models::ChatCompletionStreamOptions>>>,
    /// A list of tools the model may call. You can provide either [custom tools](/docs/guides/function-calling#custom-tools) or [function tools](/docs/guides/function-calling).
    #[serde(rename = "tools", skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<models::CreateChatCompletionRequestAllOfTools>>,
    #[serde(rename = "tool_choice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Box<models::ChatCompletionToolChoiceOption>>,
    /// Whether to enable [parallel function calling](/docs/guides/function-calling#configuring-parallel-function-calling) during tool use.
    #[serde(rename = "parallel_tool_calls", skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(rename = "function_call", skip_serializing_if = "Option::is_none")]
    pub function_call: Option<Box<models::CreateChatCompletionRequestAllOfFunctionCall>>,
    /// Deprecated in favor of `tools`.  A list of functions the model may generate JSON inputs for.
    #[serde(rename = "functions", skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<models::ChatCompletionFunctions>>,
}

impl CreateChatCompletionRequest {
    pub fn new(
        messages: Vec<models::ChatCompletionRequestMessage>,
        model: models::ModelIdsShared,
    ) -> CreateChatCompletionRequest {
        CreateChatCompletionRequest {
            messages,
            model: Box::new(model),
            metadata: None,
            top_logprobs: None,
            temperature: None,
            top_p: None,
            user: None,
            safety_identifier: None,
            prompt_cache_key: None,
            service_tier: None,
            prompt_cache_retention: None,
            modalities: None,
            verbosity: None,
            reasoning_effort: None,
            max_completion_tokens: None,
            frequency_penalty: None,
            presence_penalty: None,
            web_search_options: None,
            response_format: None,
            audio: None,
            store: None,
            stream: None,
            stop: None,
            logit_bias: None,
            logprobs: None,
            max_tokens: None,
            n: None,
            prediction: None,
            seed: None,
            stream_options: None,
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            function_call: None,
            functions: None,
        }
    }
}
// The retention policy for the prompt cache. Set to `24h` to enable extended prompt caching, which keeps cached prefixes active for longer, up to a maximum of 24 hours. [Learn more](/docs/guides/prompt-caching#prompt-cache-retention).

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateChatCompletionRequestAllOfAudio {
    /// The voice the model uses to respond. Supported built-in voices are `alloy`, `ash`, `ballad`, `coral`, `echo`, `fable`, `nova`, `onyx`, `sage`, `shimmer`, `marin`, and `cedar`. You may also provide a custom voice object with an `id`, for example `{ \"id\": \"voice_1234\" }`.
    #[serde(rename = "voice")]
    pub voice: Box<models::VoiceIdsOrCustomVoice>,
    /// Specifies the output audio format. Must be one of `wav`, `mp3`, `flac`, `opus`, or `pcm16`.
    #[serde(rename = "format")]
    pub format: ChatMetaFormat,
}

impl CreateChatCompletionRequestAllOfAudio {
    /// Parameters for audio output. Required when audio output is requested with `modalities: [\"audio\"]`. [Learn more](/docs/guides/audio).
    pub fn new(
        voice: models::VoiceIdsOrCustomVoice,
        format: ChatMetaFormat,
    ) -> CreateChatCompletionRequestAllOfAudio {
        CreateChatCompletionRequestAllOfAudio { voice: Box::new(voice), format }
    }
}
// Specifies the output audio format. Must be one of `wav`, `mp3`, `flac`, `opus`, or `pcm16`.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateChatCompletionRequestAllOfFunctionCall {
    /// `none` means the model will not call a function and instead generates a message. `auto` means the model can pick between generating a message or calling a function.
    String(String),
    ChatCompletionFunctionCallOption(Box<models::ChatCompletionFunctionCallOption>),
}

impl Default for CreateChatCompletionRequestAllOfFunctionCall {
    fn default() -> Self {
        Self::String(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CreateChatCompletionRequestAllOfResponseFormat {
    #[serde(rename = "ResponseFormatText")]
    ResponseFormatText(Box<models::ResponseFormatText>),
    #[serde(rename = "ResponseFormatJsonSchema")]
    ResponseFormatJsonSchema(Box<models::ResponseFormatJsonSchema>),
    #[serde(rename = "ResponseFormatJsonObject")]
    ResponseFormatJsonObject(Box<models::ResponseFormatJsonObject>),
}

impl Default for CreateChatCompletionRequestAllOfResponseFormat {
    fn default() -> Self {
        Self::ResponseFormatText(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateChatCompletionRequestAllOfTools {
    ChatCompletionTool(Box<models::ChatCompletionTool>),
    CustomToolChatCompletions(Box<models::CustomToolChatCompletions>),
}

impl Default for CreateChatCompletionRequestAllOfTools {
    fn default() -> Self {
        Self::ChatCompletionTool(Default::default())
    }
}
// The type of the tool. Currently, only `function` is supported.
