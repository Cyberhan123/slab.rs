use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(feature = "ggml")]
use std::collections::BTreeMap;

#[cfg(feature = "ggml")]
pub(crate) type JsonOptions = BTreeMap<String, serde_json::Value>;

#[cfg(feature = "ggml")]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

#[cfg(feature = "ggml")]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct TextGenerationUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_tokens_details: TextPromptTokensDetails,
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct TextGenerationOpOptions {
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub min_p: Option<f32>,
    #[serde(default)]
    pub presence_penalty: Option<f32>,
    #[serde(default)]
    pub repetition_penalty: Option<f32>,
    #[serde(default)]
    pub session_key: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub gbnf: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CandleLlamaLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default)]
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CandleWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CandleDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default = "default_candle_sd_version")]
    pub sd_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct OnnxLoadConfig {
    pub model_path: PathBuf,
    #[serde(default = "default_execution_providers")]
    pub execution_providers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intra_op_num_threads: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_op_num_threads: Option<usize>,
}

fn default_candle_sd_version() -> String {
    "v2-1".to_owned()
}

fn default_execution_providers() -> Vec<String> {
    vec!["CPU".to_owned()]
}
