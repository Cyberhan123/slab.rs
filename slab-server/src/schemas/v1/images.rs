use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Generation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImageMode {
    /// Text-to-image (default).
    #[default]
    Txt2Img,
    /// Image-to-image (requires `init_image`).
    Img2Img,
}

/// Request body for `POST /v1/images/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImageGenerationRequest {
    /// The model identifier to use.
    pub model: String,

    /// Text description of the desired image.
    pub prompt: String,

    /// Negative text prompt (things to avoid in the generated image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,

    /// Number of images to generate (default `1`).
    #[serde(default = "default_n")]
    pub n: u32,

    /// Output image width in pixels (default `512`).
    #[serde(default = "default_width")]
    pub width: u32,

    /// Output image height in pixels (default `512`).
    #[serde(default = "default_height")]
    pub height: u32,

    /// Classifier-Free Guidance scale (default `7.0`).
    #[serde(default = "default_cfg_scale", skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,

    /// Distilled guidance (Flux/SD3 models, default `3.5`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<f32>,

    /// Number of denoising steps (default `20`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<i32>,

    /// RNG seed (`-1` = random, default `42`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Sampling method (`"euler"`, `"euler_a"`, `"lcm"`, etc., `"auto"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<String>,

    /// Sigma schedule (`"discrete"`, `"karras"`, etc., `"auto"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,

    /// CLIP skip layers (default `0` = auto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_skip: Option<i32>,

    /// DDIM eta (default `0.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<f32>,

    /// Init-image influence strength for img2img (0–1, default `0.75`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,

    /// Init image as a base64-encoded data URI (`data:image/png;base64,...`).
    /// Required when `mode` is `img2img`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_image: Option<String>,

    /// Generation mode (default `txt2img`).
    #[serde(default)]
    pub mode: ImageMode,
}

fn default_n() -> u32 {
    1
}
fn default_width() -> u32 {
    512
}
fn default_height() -> u32 {
    512
}
fn default_cfg_scale() -> Option<f32> {
    None
}
