use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::domain::models::{
    AudioTranscriptionCommand, AudioTranscriptionTaskView, TranscribeDecodeOptions,
    TranscribeVadOptions,
};
use crate::schemas::tasks::{TaskProgressResponse, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct AudioTranscriptionRequest {
    /// Optional catalog model identifier used for history attribution.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "model_id must not be empty"
    ))]
    pub model_id: Option<String>,
    /// The audio file path to transcribe.
    #[validate(custom(
        function = "crate::schemas::validation::validate_absolute_path",
        message = "path must be an absolute path without '..'"
    ))]
    pub path: String,
    /// Optional language override passed to whisper inference.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub language: Option<String>,
    /// Optional initial prompt passed to whisper inference.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prompt: Option<String>,
    /// Enable whisper language auto-detection when no explicit language is set.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detect_language: Option<bool>,
    /// Optional VAD (Voice Activity Detection) settings.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(nested)]
    pub vad: Option<TranscribeVadRequest>,
    /// Optional whisper decoding settings.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(nested)]
    pub decode: Option<TranscribeDecodeRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_vad_request"))]
pub struct TranscribeVadRequest {
    /// Enable VAD during whisper transcription.
    #[serde(default)]
    pub enabled: bool,
    /// Absolute path to the VAD model file.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(custom(
        function = "crate::schemas::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: Option<String>,
    /// Probability threshold used to classify speech.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0.0, max = 1.0, message = "threshold must be between 0.0 and 1.0"))]
    pub threshold: Option<f32>,
    /// Minimum speech segment duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "min_speech_duration_ms must be >= 0"))]
    pub min_speech_duration_ms: Option<i32>,
    /// Minimum silence duration in milliseconds used to split segments.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "min_silence_duration_ms must be >= 0"))]
    pub min_silence_duration_ms: Option<i32>,
    /// Maximum speech segment duration in seconds before auto-splitting.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_speech_duration_s: Option<f32>,
    /// Padding in milliseconds added around each detected speech segment.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "speech_pad_ms must be >= 0"))]
    pub speech_pad_ms: Option<i32>,
    /// Overlap in seconds between adjacent VAD segments.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0.0, message = "samples_overlap must be >= 0.0"))]
    pub samples_overlap: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct TranscribeDecodeRequest {
    /// Start offset in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "offset_ms must be >= 0"))]
    pub offset_ms: Option<i32>,
    /// Duration in milliseconds to process (0 means full input).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "duration_ms must be >= 0"))]
    pub duration_ms: Option<i32>,
    /// Do not use past transcription as prompt.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub no_context: Option<bool>,
    /// Do not generate timestamps.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub no_timestamps: Option<bool>,
    /// Enable token-level timestamps.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub token_timestamps: Option<bool>,
    /// Split timestamps on words instead of tokens.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub split_on_word: Option<bool>,
    /// Suppress non-speech tokens.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suppress_nst: Option<bool>,
    /// Word timestamp probability threshold.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0.0, max = 1.0, message = "word_thold must be between 0.0 and 1.0"))]
    pub word_thold: Option<f32>,
    /// Maximum segment length in characters.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "max_len must be >= 0"))]
    pub max_len: Option<i32>,
    /// Maximum tokens per segment.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0, message = "max_tokens must be >= 0"))]
    pub max_tokens: Option<i32>,
    /// Initial decoding temperature.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0.0, message = "temperature must be >= 0.0"))]
    pub temperature: Option<f32>,
    /// Temperature increment for fallback decoding.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(min = 0.0, message = "temperature_inc must be >= 0.0"))]
    pub temperature_inc: Option<f32>,
    /// Entropy threshold.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub entropy_thold: Option<f32>,
    /// Log probability threshold.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub logprob_thold: Option<f32>,
    /// No-speech threshold.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub no_speech_thold: Option<f32>,
    /// Enable tinydiarize speaker turn detection.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tdrz_enable: Option<bool>,
}

fn validate_vad_request(request: &TranscribeVadRequest) -> Result<(), ValidationError> {
    if request.enabled
        && request.model_path.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_none()
    {
        let mut error = ValidationError::new("missing_model_path");
        error.message = Some("model_path is required when VAD is enabled".into());
        return Err(error);
    }

    if request.max_speech_duration_s.is_some_and(|value| value <= 0.0) {
        let mut error = ValidationError::new("max_speech_duration_s");
        error.message = Some("max_speech_duration_s must be > 0.0".into());
        return Err(error);
    }

    Ok(())
}

impl From<TranscribeVadRequest> for TranscribeVadOptions {
    fn from(request: TranscribeVadRequest) -> Self {
        Self {
            enabled: request.enabled,
            model_path: request.model_path,
            threshold: request.threshold,
            min_speech_duration_ms: request.min_speech_duration_ms,
            min_silence_duration_ms: request.min_silence_duration_ms,
            max_speech_duration_s: request.max_speech_duration_s,
            speech_pad_ms: request.speech_pad_ms,
            samples_overlap: request.samples_overlap,
        }
    }
}

impl From<TranscribeDecodeRequest> for TranscribeDecodeOptions {
    fn from(request: TranscribeDecodeRequest) -> Self {
        Self {
            offset_ms: request.offset_ms,
            duration_ms: request.duration_ms,
            no_context: request.no_context,
            no_timestamps: request.no_timestamps,
            token_timestamps: request.token_timestamps,
            split_on_word: request.split_on_word,
            suppress_nst: request.suppress_nst,
            word_thold: request.word_thold,
            max_len: request.max_len,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            temperature_inc: request.temperature_inc,
            entropy_thold: request.entropy_thold,
            logprob_thold: request.logprob_thold,
            no_speech_thold: request.no_speech_thold,
            tdrz_enable: request.tdrz_enable,
        }
    }
}

impl From<AudioTranscriptionRequest> for AudioTranscriptionCommand {
    fn from(request: AudioTranscriptionRequest) -> Self {
        Self {
            model_id: normalize_optional_text(request.model_id),
            path: request.path,
            language: normalize_optional_text(request.language),
            prompt: normalize_optional_text(request.prompt),
            detect_language: request.detect_language,
            vad: request.vad.map(Into::into),
            decode: request.decode.map(Into::into),
        }
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AudioTranscriptionTaskResponse {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<TaskProgressResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    pub backend_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub source_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect_language: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vad_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_text: Option<String>,
    pub request_data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_data: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<AudioTranscriptionTaskView> for AudioTranscriptionTaskResponse {
    fn from(value: AudioTranscriptionTaskView) -> Self {
        Self {
            task_id: value.task_id,
            task_type: value.task_type,
            status: value.status.into(),
            progress: value.progress.map(Into::into),
            error_msg: value.error_msg,
            backend_id: value.backend_id,
            model_id: value.model_id,
            source_path: value.source_path,
            language: value.language,
            prompt: value.prompt,
            detect_language: value.detect_language,
            vad_json: value.vad_json,
            decode_json: value.decode_json,
            transcript_text: value.transcript_text,
            request_data: value.request_data,
            result_data: value.result_data,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
