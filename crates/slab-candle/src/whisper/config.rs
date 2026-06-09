use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use slab_types::RuntimeDevicePreference;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WhisperWeightSource {
    Safetensors,
    QuantizedGguf,
}

impl Default for WhisperWeightSource {
    fn default() -> Self {
        Self::Safetensors
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WhisperTask {
    Transcribe,
    Translate,
}

impl Default for WhisperTask {
    fn default() -> Self {
        Self::Transcribe
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandleWhisperLoadConfig {
    pub model_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<RuntimeDevicePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mel_filters_path: Option<PathBuf>,
    #[serde(default)]
    pub weight_source: WhisperWeightSource,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionRequest {
    pub samples: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub detect_language: bool,
    #[serde(default)]
    pub task: WhisperTask,
    #[serde(default)]
    pub timestamps: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub temperature_fallback: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_initial_timestamp_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_ratio_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprob_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptionSegment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TranscriptionResponse {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_language: Option<String>,
    #[serde(default)]
    pub segments: Vec<TranscriptionSegment>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcription_request_defaults_to_transcribe_without_timestamps() {
        let request = TranscriptionRequest::default();
        assert_eq!(request.task, WhisperTask::Transcribe);
        assert!(!request.timestamps);
    }
}
