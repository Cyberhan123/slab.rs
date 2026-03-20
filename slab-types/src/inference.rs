use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::whisper::{WhisperDecodeOptions, WhisperVadOptions};

pub type JsonOptions = BTreeMap<String, serde_json::Value>;

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
    pub session_key: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationResponse {
    pub text: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub tokens_used: Option<u32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct TextGenerationChunk {
    pub delta: String,
    #[serde(default)]
    pub done: bool,
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
    pub vad: Option<WhisperVadOptions>,
    #[serde(default)]
    pub decode: Option<WhisperDecodeOptions>,
    #[serde(default)]
    pub options: JsonOptions,
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
/// [`crate::diffusion::DiffusionImageRequest`]; prefer `DiffusionImageRequest` for
/// richer diffusion-specific options. The numeric types here (`steps: Option<i32>`,
/// `guidance: Option<f32>`) are intentionally aligned with those of `DiffusionImageRequest`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub steps: Option<i32>,
    #[serde(default)]
    pub guidance: Option<f32>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for ImageGenerationRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: Some(20),
            guidance: Some(7.5),
            seed: None,
            options: JsonOptions::default(),
        }
    }
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
