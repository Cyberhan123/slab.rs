use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
