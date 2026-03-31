use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Typed `model.load` payload for the `ggml.llama` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GgmlLlamaLoadConfig {
    pub model_path: PathBuf,
    pub num_workers: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u32>,
}

/// Typed `model.load` payload for the `ggml.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GgmlWhisperLoadConfig {
    pub model_path: PathBuf,
}

/// Typed `model.load` payload for the `ggml.diffusion` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GgmlDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusion_model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taesd_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_l_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_g_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t5xxl_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_vision_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_net_path: Option<PathBuf>,
    #[serde(default)]
    pub flash_attn: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_device: Option<String>,
    #[serde(default)]
    pub offload_params_to_cpu: bool,
    #[serde(default)]
    pub enable_mmap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_threads: Option<i32>,
}

/// Typed `model.load` payload for the `candle.llama` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleLlamaLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default)]
    pub seed: u64,
}

/// Typed `model.load` payload for the `candle.whisper` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
}

/// Typed `model.load` payload for the `candle.diffusion` backend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleDiffusionLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vae_path: Option<PathBuf>,
    #[serde(default = "default_candle_sd_version")]
    pub sd_version: String,
}

/// Typed `model.load` payload for the `onnx.*` backends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OnnxLoadConfig {
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
