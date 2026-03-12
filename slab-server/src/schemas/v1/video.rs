use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for `POST /v1/video/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VideoGenerationRequest {
    /// The model identifier to use.
    pub model: String,

    /// Text description of the desired video content.
    pub prompt: String,

    /// Negative text prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,

    /// Frame width in pixels (default `512`).
    #[serde(default = "default_width")]
    pub width: u32,

    /// Frame height in pixels (default `512`).
    #[serde(default = "default_height")]
    pub height: u32,

    /// Number of video frames to generate (default `16`).
    #[serde(default = "default_frames")]
    pub video_frames: i32,

    /// Output frames per second (default `8`).
    #[serde(default = "default_fps")]
    pub fps: f32,

    /// Classifier-Free Guidance scale (default `7.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,

    /// Distilled guidance (default `3.5`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<f32>,

    /// Number of denoising steps (default `20`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<i32>,

    /// RNG seed (default `42`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Sampling method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<String>,

    /// Sigma schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,

    /// Init-image for video2video (base64 data URI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_image: Option<String>,

    /// Strength for init-image influence (default `0.75`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
}

fn default_width() -> u32 { 512 }
fn default_height() -> u32 { 512 }
fn default_frames() -> i32 { 16 }
fn default_fps() -> f32 { 8.0 }
