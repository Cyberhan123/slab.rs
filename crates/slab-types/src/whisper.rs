use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Voice activity detection options for whisper transcription.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WhisperVadOptions {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<WhisperVadParams>,
}

/// Fine-grained whisper VAD tuning knobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WhisperVadParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_speech_duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_silence_duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_speech_duration_s: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech_pad_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples_overlap: Option<f32>,
}

/// Whisper decode and timestamping options.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct WhisperDecodeOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_context: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_timestamps: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_on_word: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_nst: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_len: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_inc: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entropy_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logprob_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_speech_thold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdrz_enable: Option<bool>,
}
