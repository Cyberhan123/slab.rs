use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::inference::JsonOptions;
use crate::media::{GeneratedFrame, GeneratedImage, RawImageInput};

/// Normalized diffusion image request independent of HTTP and protobuf DTOs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionImageRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    #[serde(default = "default_count")]
    pub count: u32,
    pub width: u32,
    pub height: u32,
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
    #[serde(default)]
    pub init_image: Option<RawImageInput>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for DiffusionImageRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            count: default_count(),
            width: 512,
            height: 512,
            cfg_scale: None,
            guidance: None,
            steps: None,
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

/// Generated diffusion image artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionImageResponse {
    #[serde(default)]
    pub images: Vec<GeneratedImage>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

/// Normalized diffusion video request independent of HTTP and protobuf DTOs.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionVideoRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_video_frames")]
    pub video_frames: i32,
    #[serde(default = "default_fps")]
    pub fps: f32,
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
    #[serde(default)]
    pub init_image: Option<RawImageInput>,
    #[serde(default)]
    pub options: JsonOptions,
}

impl Default for DiffusionVideoRequest {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: None,
            width: 512,
            height: 512,
            video_frames: default_video_frames(),
            fps: default_fps(),
            cfg_scale: None,
            guidance: None,
            steps: None,
            seed: None,
            sample_method: None,
            scheduler: None,
            strength: None,
            init_image: None,
            options: JsonOptions::default(),
        }
    }
}

/// Generated raw frames returned by a diffusion video pipeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DiffusionVideoResponse {
    #[serde(default)]
    pub frames: Vec<GeneratedFrame>,
    #[serde(default)]
    pub metadata: JsonOptions,
}

const fn default_count() -> u32 {
    1
}

const fn default_video_frames() -> i32 {
    16
}

const fn default_fps() -> f32 {
    8.0
}
