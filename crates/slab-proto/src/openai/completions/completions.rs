use serde::{Deserialize, Serialize};

use crate::openai::models::_stubs::CompletionUsage;
use crate::openai::models::chat::completion::stream::ChatCompletionStreamOptions;
use crate::openai::models::common::misc::StopConfiguration;

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateCompletionRequest {
    #[serde(rename = "model")]
    pub model: CreateCompletionRequestModel,
    #[serde(rename = "prompt", skip_serializing_if = "Option::is_none")]
    pub prompt: Option<serde_json::Value>,
    #[serde(rename = "best_of", skip_serializing_if = "Option::is_none")]
    pub best_of: Option<i32>,
    #[serde(rename = "echo", skip_serializing_if = "Option::is_none")]
    pub echo: Option<bool>,
    #[serde(rename = "frequency_penalty", skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(rename = "logit_bias", skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::HashMap<String, i32>>,
    #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<i32>,
    #[serde(rename = "max_tokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    #[serde(rename = "presence_penalty", skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(rename = "seed", skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(rename = "stop", skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopConfiguration>,
    #[serde(rename = "stream", skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(rename = "stream_options", skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<ChatCompletionStreamOptions>,
    #[serde(rename = "suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "temperature", skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(rename = "top_p", skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateCompletionRequestModel {
    ModelEnum(CreateCompletionRequestModelEnum),
    StringValue(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum CreateCompletionRequestModelEnum {
    #[serde(rename = "gpt-3.5-turbo-instruct")]
    #[default]
    Gpt35TurboInstruct,
    #[serde(rename = "davinci-002")]
    Davinci002,
    #[serde(rename = "babbage-002")]
    Babbage002,
}


impl Default for CreateCompletionRequestModel {
    fn default() -> Self {
        Self::StringValue(String::new())
    }
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateCompletionResponse {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "object")]
    pub object: CompletionResponseObject,
    #[serde(rename = "created")]
    pub created: i32,
    #[serde(rename = "model")]
    pub model: String,
    #[serde(rename = "system_fingerprint", skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(rename = "choices")]
    pub choices: Vec<CreateCompletionResponseChoicesInner>,
    #[serde(rename = "usage", skip_serializing_if = "Option::is_none")]
    pub usage: Option<CompletionUsage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum CompletionResponseObject {
    #[serde(rename = "text_completion")]
    #[default]
    TextCompletion,
}


#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreateCompletionResponseChoicesInner {
    #[serde(rename = "finish_reason", skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<CompletionFinishReason>,
    #[serde(rename = "index")]
    pub index: i32,
    #[serde(rename = "logprobs", skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
    #[serde(rename = "text")]
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum CompletionFinishReason {
    #[serde(rename = "stop")]
    #[default]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "content_filter")]
    ContentFilter,
}

