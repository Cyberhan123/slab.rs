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
