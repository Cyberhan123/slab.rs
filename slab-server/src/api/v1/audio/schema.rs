use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
pub struct CompletionRequest {
    /// The audio file path to transcribe.
    #[validate(custom(
        function = "crate::api::validation::validate_absolute_path",
        message = "path must be an absolute path without '..'"
    ))]
    pub path: String,
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
        function = "crate::api::validation::validate_absolute_path",
        message = "model_path must be an absolute path without '..'"
    ))]
    pub model_path: Option<String>,
    /// Probability threshold used to classify speech.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[validate(range(
        min = 0.0,
        max = 1.0,
        message = "threshold must be between 0.0 and 1.0"
    ))]
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
    #[validate(range(
        min = 0.0,
        max = 1.0,
        message = "word_thold must be between 0.0 and 1.0"
    ))]
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
        && request
            .model_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        let mut error = ValidationError::new("missing_model_path");
        error.message = Some("model_path is required when VAD is enabled".into());
        return Err(error);
    }

    if request
        .max_speech_duration_s
        .is_some_and(|value| value <= 0.0)
    {
        let mut error = ValidationError::new("max_speech_duration_s");
        error.message = Some("max_speech_duration_s must be > 0.0".into());
        return Err(error);
    }

    Ok(())
}
