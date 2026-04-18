use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::ValidationError;
use crate::media::RawImageInput;
use crate::whisper::{WhisperDecodeOptions, WhisperVadOptions};

pub type JsonOptions = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TextPromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TextGenerationUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
    #[serde(default)]
    pub prompt_tokens_details: TextPromptTokensDetails,
    /// When `true`, the counts are best-effort estimates rather than exact
    /// tokenizer-native values reported by the backend.
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
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
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl TextGenerationRequest {
    /// Validates the request parameters according to typical inference constraints.
    ///
    /// # Errors
    ///
    /// Returns `Err` if any parameter is outside its valid range:
    /// - `temperature`: must be in [0.0, 2.0]
    /// - `top_p`: must be in [0.0, 1.0]
    /// - `top_k`: must be >= 0
    /// - `min_p`: must be in [0.0, 1.0]
    /// - `presence_penalty`: must be in [-2.0, 2.0]
    /// - `repetition_penalty`: must be in [-2.0, 2.0]
    /// - `max_tokens`: must be > 0 if specified
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(temp) = self.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err(ValidationError::TemperatureOutOfRange(temp));
            }
        }

        if let Some(top_p) = self.top_p {
            if !(0.0..=1.0).contains(&top_p) {
                return Err(ValidationError::TopPOutOfRange(top_p));
            }
        }

        if let Some(top_k) = self.top_k {
            if top_k < 0 {
                return Err(ValidationError::TopKOutOfRange(top_k));
            }
        }

        if let Some(min_p) = self.min_p {
            if !(0.0..=1.0).contains(&min_p) {
                return Err(ValidationError::MinPOutOfRange(min_p));
            }
        }

        if let Some(presence) = self.presence_penalty {
            if !(-2.0..=2.0).contains(&presence) {
                return Err(ValidationError::PresencePenaltyOutOfRange(presence));
            }
        }

        if let Some(repetition) = self.repetition_penalty {
            if !(-2.0..=2.0).contains(&repetition) {
                return Err(ValidationError::FrequencyPenaltyOutOfRange(repetition));
            }
        }

        if let Some(max_tokens) = self.max_tokens {
            if max_tokens == 0 {
                return Err(ValidationError::MaxTokensOutOfRange(max_tokens));
            }
        }

        Ok(())
    }
}

/// Fully typed backend options derived from a text-generation request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationOpOptions {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationResponse {
    pub text: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub tokens_used: Option<u32>,
    #[serde(default)]
    pub usage: Option<TextGenerationUsage>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationChunk {
    pub delta: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<TextGenerationUsage>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AudioTranscriptionRequest {
    pub audio_path: PathBuf,
    /// In-process PCM audio samples populated by the runtime after audio decoding.
    /// This field is intentionally skipped during serde serialization/deserialization
    /// because it is never transported over wire (HTTP/gRPC); it is only used
    /// in-process within slab-runtime after the audio file has been decoded.
    #[serde(default, skip_serializing, skip_deserializing)]
    #[schemars(skip)]
    pub pcm_samples: Option<Arc<[f32]>>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub detect_language: Option<bool>,
    #[serde(default)]
    pub vad: Option<WhisperVadOptions>,
    #[serde(default)]
    pub decode: Option<WhisperDecodeOptions>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl AudioTranscriptionRequest {
    /// Validates the audio transcription request parameters.
    ///
    /// # Errors
    ///
    /// Returns `Err` if any parameter is invalid:
    /// - `language`: must be a valid ISO 639-1 or ISO 639-2 language code if specified
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(ref lang) = self.language {
            if !is_valid_language_code(lang) {
                return Err(ValidationError::InvalidIso639LanguageCode(lang.clone()));
            }
        }

        Ok(())
    }
}

/// Checks if a string is a valid ISO 639-1 (2-letter) or ISO 639-2 (3-letter) language code.
fn is_valid_language_code(code: &str) -> bool {
    let code = code.trim().to_lowercase();

    // ISO 639-1: 2-letter codes
    if code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        return true;
    }

    // ISO 639-2: 3-letter codes
    if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        return true;
    }

    false
}

/// Fully typed backend options derived from an audio-transcription request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AudioTranscriptionOpOptions {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub detect_language: Option<bool>,
    #[serde(default)]
    pub vad: Option<WhisperVadOptions>,
    #[serde(default)]
    pub decode: Option<WhisperDecodeOptions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AudioTranscriptionResponse {
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

/// High-level image generation request. This is the transport-layer counterpart to
/// [`crate::diffusion::DiffusionImageRequest`]; prefer `DiffusionImageRequest` when
/// you need the shared/common diffusion envelope plus backend-specific parameter groups.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default = "default_image_count")]
    pub count: u32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub cfg_scale: Option<f32>,
    #[serde(default)]
    pub steps: Option<i32>,
    #[serde(default)]
    pub guidance: Option<f32>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub sample_method: Option<String>,
    #[serde(default)]
    pub scheduler: Option<String>,
    #[serde(default)]
    pub clip_skip: Option<i32>,
    #[serde(default)]
    pub strength: Option<f32>,
    #[serde(default)]
    pub eta: Option<f32>,
    #[serde(default)]
    pub init_image: Option<RawImageInput>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            count: default_image_count(),
            width: 512,
            height: 512,
            cfg_scale: None,
            steps: Some(20),
            guidance: Some(7.5),
            seed: None,
            sample_method: None,
            scheduler: None,
            clip_skip: None,
            strength: None,
            eta: None,
            init_image: None,
            options: JsonOptions::default(),
        }
    }
}

impl ImageGenerationRequest {
    /// Validates the image generation request parameters.
    ///
    /// # Errors
    ///
    /// Returns `Err` if any parameter is outside its valid range:
    /// - `width`: must be in [64, 4096]
    /// - `height`: must be in [64, 4096]
    /// - `count`: must be > 0
    /// - `prompt`: cannot be empty
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.prompt.trim().is_empty() {
            return Err(ValidationError::EmptyPrompt);
        }

        if !(64..=4096).contains(&self.width) {
            return Err(ValidationError::WidthOutOfRange(self.width));
        }

        if !(64..=4096).contains(&self.height) {
            return Err(ValidationError::HeightOutOfRange(self.height));
        }

        if self.count == 0 {
            return Err(ValidationError::CountOutOfRange(self.count));
        }

        Ok(())
    }
}

const fn default_image_count() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImageGenerationResponse {
    #[serde(default)]
    pub images: Vec<Vec<u8>>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImageEmbeddingRequest {
    #[serde(default)]
    pub image: Vec<u8>,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImageEmbeddingResponse {
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[cfg(test)]
mod tests {
    use super::*;

    // TextGenerationRequest validation tests
    #[test]
    fn text_generation_request_validate_accepts_valid_values() {
        let request = TextGenerationRequest {
            prompt: "Hello, world!".to_string(),
            temperature: Some(1.0),
            top_p: Some(0.9),
            top_k: Some(40),
            min_p: Some(0.5),
            presence_penalty: Some(0.5),
            repetition_penalty: Some(1.0),
            max_tokens: Some(100),
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn text_generation_request_validate_rejects_negative_temperature() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            temperature: Some(-0.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::TemperatureOutOfRange(-0.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_temperature_above_max() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            temperature: Some(2.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::TemperatureOutOfRange(2.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_accepts_temperature_boundaries() {
        let request_min = TextGenerationRequest {
            prompt: "Hello".to_string(),
            temperature: Some(0.0),
            ..Default::default()
        };

        let request_max = TextGenerationRequest {
            prompt: "Hello".to_string(),
            temperature: Some(2.0),
            ..Default::default()
        };

        assert!(request_min.validate().is_ok());
        assert!(request_max.validate().is_ok());
    }

    #[test]
    fn text_generation_request_validate_rejects_invalid_top_p() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            top_p: Some(1.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::TopPOutOfRange(1.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_negative_top_k() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            top_k: Some(-1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::TopKOutOfRange(-1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_invalid_min_p() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            min_p: Some(-0.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::MinPOutOfRange(-0.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_invalid_presence_penalty() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            presence_penalty: Some(2.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::PresencePenaltyOutOfRange(2.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_invalid_repetition_penalty() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            repetition_penalty: Some(-2.1),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::FrequencyPenaltyOutOfRange(-2.1))
        ));
    }

    #[test]
    fn text_generation_request_validate_rejects_zero_max_tokens() {
        let request = TextGenerationRequest {
            prompt: "Hello".to_string(),
            max_tokens: Some(0),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::MaxTokensOutOfRange(0))
        ));
    }

    // ImageGenerationRequest validation tests
    #[test]
    fn image_generation_request_validate_accepts_valid_values() {
        let request = ImageGenerationRequest {
            prompt: "A beautiful landscape".to_string(),
            width: 1024,
            height: 768,
            count: 1,
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn image_generation_request_validate_rejects_empty_prompt() {
        let request = ImageGenerationRequest {
            prompt: "   ".to_string(),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::EmptyPrompt)
        ));
    }

    #[test]
    fn image_generation_request_validate_rejects_width_too_small() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            width: 63,
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::WidthOutOfRange(63))
        ));
    }

    #[test]
    fn image_generation_request_validate_rejects_width_too_large() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            width: 4097,
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::WidthOutOfRange(4097))
        ));
    }

    #[test]
    fn image_generation_request_validate_rejects_height_too_small() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            height: 63,
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::HeightOutOfRange(63))
        ));
    }

    #[test]
    fn image_generation_request_validate_rejects_height_too_large() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            height: 4097,
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::HeightOutOfRange(4097))
        ));
    }

    #[test]
    fn image_generation_request_validate_rejects_zero_count() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            count: 0,
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::CountOutOfRange(0))
        ));
    }

    #[test]
    fn image_generation_request_validate_accepts_boundary_dimensions() {
        let request_min = ImageGenerationRequest {
            prompt: "Test".to_string(),
            width: 64,
            height: 64,
            ..Default::default()
        };

        let request_max = ImageGenerationRequest {
            prompt: "Test".to_string(),
            width: 4096,
            height: 4096,
            ..Default::default()
        };

        assert!(request_min.validate().is_ok());
        assert!(request_max.validate().is_ok());
    }

    // AudioTranscriptionRequest validation tests
    #[test]
    fn audio_transcription_request_validate_accepts_valid_language_code() {
        let request = AudioTranscriptionRequest {
            audio_path: PathBuf::from("/path/to/audio.wav"),
            language: Some("en".to_string()),
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn audio_transcription_request_validate_accepts_three_letter_language_code() {
        let request = AudioTranscriptionRequest {
            audio_path: PathBuf::from("/path/to/audio.wav"),
            language: Some("eng".to_string()),
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn audio_transcription_request_validate_rejects_invalid_language_code() {
        let request = AudioTranscriptionRequest {
            audio_path: PathBuf::from("/path/to/audio.wav"),
            language: Some("invalid".to_string()),
            ..Default::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ValidationError::InvalidIso639LanguageCode(_))
        ));
    }

    #[test]
    fn audio_transcription_request_validate_accepts_none_language() {
        let request = AudioTranscriptionRequest {
            audio_path: PathBuf::from("/path/to/audio.wav"),
            language: None,
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    #[test]
    fn audio_transcription_request_validate_normalizes_case() {
        let request = AudioTranscriptionRequest {
            audio_path: PathBuf::from("/path/to/audio.wav"),
            language: Some("EN".to_string()),
            ..Default::default()
        };

        assert!(request.validate().is_ok());
    }

    // Helper function tests
    #[test]
    fn test_is_valid_language_code_with_valid_two_letter() {
        assert!(is_valid_language_code("en"));
        assert!(is_valid_language_code("fr"));
        assert!(is_valid_language_code("de"));
        assert!(is_valid_language_code("zh"));
    }

    #[test]
    fn test_is_valid_language_code_with_valid_three_letter() {
        assert!(is_valid_language_code("eng"));
        assert!(is_valid_language_code("fra"));
        assert!(is_valid_language_code("deu"));
        assert!(is_valid_language_code("zho"));
    }

    #[test]
    fn test_is_valid_language_code_with_invalid_codes() {
        assert!(!is_valid_language_code("e"));
        assert!(!is_valid_language_code("english"));
        assert!(!is_valid_language_code("123"));
        assert!(!is_valid_language_code(""));
        assert!(!is_valid_language_code("en1"));
    }
}
