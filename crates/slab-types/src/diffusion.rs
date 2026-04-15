use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::inference::JsonOptions;
use crate::media::{GeneratedFrame, GeneratedImage, RawImageInput};

/// Shared diffusion request fields carried across image/video backends.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionRequestCommon {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub init_image: Option<RawImageInput>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for DiffusionRequestCommon {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            width: 512,
            height: 512,
            init_image: None,
            options: JsonOptions::default(),
        }
    }
}

/// GGML diffusion image-generation parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GgmlDiffusionImageParams {
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub cfg_scale: Option<f32>,
    #[serde(default)]
    pub guidance: Option<f32>,
    #[serde(default)]
    pub steps: Option<i32>,
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
}

/// Backend-specific image-generation parameters kept distinct from shared diffusion fields.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "backend", content = "params", rename_all = "snake_case")]
pub enum DiffusionImageBackend {
    Ggml(GgmlDiffusionImageParams),
}

impl Default for DiffusionImageBackend {
    fn default() -> Self {
        Self::Ggml(GgmlDiffusionImageParams::default())
    }
}

impl DiffusionImageBackend {
    pub fn as_ggml(&self) -> &GgmlDiffusionImageParams {
        match self {
            Self::Ggml(params) => params,
        }
    }
}

/// Normalized diffusion image request independent of HTTP and protobuf DTOs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionImageRequest {
    pub common: DiffusionRequestCommon,
    #[serde(default)]
    pub backend: DiffusionImageBackend,
}

/// Generated diffusion image artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionImageResponse {
    #[serde(default)]
    pub images: Vec<GeneratedImage>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

/// GGML diffusion video-generation parameters.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct GgmlDiffusionVideoParams {
    #[serde(default)]
    pub video_frames: Option<i32>,
    #[serde(default)]
    pub fps: Option<f32>,
    #[serde(default)]
    pub cfg_scale: Option<f32>,
    #[serde(default)]
    pub guidance: Option<f32>,
    #[serde(default)]
    pub steps: Option<i32>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub sample_method: Option<String>,
    #[serde(default)]
    pub scheduler: Option<String>,
    #[serde(default)]
    pub strength: Option<f32>,
}

/// Backend-specific video-generation parameters kept distinct from shared diffusion fields.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "backend", content = "params", rename_all = "snake_case")]
pub enum DiffusionVideoBackend {
    Ggml(GgmlDiffusionVideoParams),
}

impl Default for DiffusionVideoBackend {
    fn default() -> Self {
        Self::Ggml(GgmlDiffusionVideoParams::default())
    }
}

impl DiffusionVideoBackend {
    pub fn as_ggml(&self) -> &GgmlDiffusionVideoParams {
        match self {
            Self::Ggml(params) => params,
        }
    }
}

/// Normalized diffusion video request independent of HTTP and protobuf DTOs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionVideoRequest {
    pub common: DiffusionRequestCommon,
    #[serde(default)]
    pub backend: DiffusionVideoBackend,
}

/// Generated raw frames returned by a diffusion video pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionVideoResponse {
    #[serde(default)]
    pub frames: Vec<GeneratedFrame>,
    #[serde(default)]
    pub metadata: JsonOptions,
}
