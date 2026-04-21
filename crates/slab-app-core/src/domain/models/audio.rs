use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTranscriptionCommand {
    pub model_id: Option<String>,
    pub path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad: Option<TranscribeVadOptions>,
    pub decode: Option<TranscribeDecodeOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeVadOptions {
    pub enabled: bool,
    pub model_path: Option<String>,
    pub threshold: Option<f32>,
    pub min_speech_duration_ms: Option<i32>,
    pub min_silence_duration_ms: Option<i32>,
    pub max_speech_duration_s: Option<f32>,
    pub speech_pad_ms: Option<i32>,
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscribeDecodeOptions {
    pub offset_ms: Option<i32>,
    pub duration_ms: Option<i32>,
    pub no_context: Option<bool>,
    pub no_timestamps: Option<bool>,
    pub token_timestamps: Option<bool>,
    pub split_on_word: Option<bool>,
    pub suppress_nst: Option<bool>,
    pub word_thold: Option<f32>,
    pub max_len: Option<i32>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub temperature_inc: Option<f32>,
    pub entropy_thold: Option<f32>,
    pub logprob_thold: Option<f32>,
    pub no_speech_thold: Option<f32>,
    pub tdrz_enable: Option<bool>,
}
