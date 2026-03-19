use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub type JsonOptions = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationResponse {
    pub text: String,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub tokens_used: Option<u32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TextGenerationChunk {
    pub delta: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioTranscriptionRequest {
    pub audio_path: PathBuf,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AudioTranscriptionResponse {
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub guidance: f32,
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
            steps: 20,
            guidance: 7.5,
            seed: None,
            options: JsonOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageGenerationResponse {
    #[serde(default)]
    pub images: Vec<Vec<u8>>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageEmbeddingRequest {
    #[serde(default)]
    pub image: Vec<u8>,
    #[serde(default)]
    pub options: JsonOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImageEmbeddingResponse {
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub metadata: JsonOptions,
}
