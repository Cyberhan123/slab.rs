use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

const MAX_PROMPT_BYTES: usize = 128 * 1024;
const MAX_IMAGES_PER_REQUEST: u32 = 10;
const MAX_IMAGE_DIM: u32 = 2048;

/// Generation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
pub enum ImageMode {
    /// Text-to-image (default).
    #[default]
    #[serde(rename = "txt2img")]
    Txt2Img,
    /// Image-to-image (requires `init_image`).
    #[serde(rename = "img2img")]
    Img2Img,
}

/// Request body for `POST /v1/images/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_image_generation_request"))]
pub struct ImageGenerationRequest {
    /// The model identifier to use.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "model must not be empty"
    ))]
    pub model: String,

    /// Text description of the desired image.
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "prompt must not be empty"
    ))]
    pub prompt: String,

    /// Negative text prompt (things to avoid in the generated image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "negative_prompt must not be empty"
    ))]
    pub negative_prompt: Option<String>,

    /// Number of images to generate (default `1`).
    #[serde(default = "default_n")]
    #[validate(range(min = 1, max = 10, message = "n must be between 1 and 10"))]
    pub n: u32,

    /// Output image width in pixels (default `512`).
    #[serde(default = "default_width")]
    #[validate(range(min = 1, max = 2048, message = "width must be between 1 and 2048"))]
    pub width: u32,

    /// Output image height in pixels (default `512`).
    #[serde(default = "default_height")]
    #[validate(range(min = 1, max = 2048, message = "height must be between 1 and 2048"))]
    pub height: u32,

    /// Classifier-Free Guidance scale (default `7.0`).
    #[serde(default = "default_cfg_scale", skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, message = "cfg_scale must be >= 0.0"))]
    pub cfg_scale: Option<f32>,

    /// Distilled guidance (Flux/SD3 models, default `3.5`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, message = "guidance must be >= 0.0"))]
    pub guidance: Option<f32>,

    /// Number of denoising steps (default `20`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, message = "steps must be at least 1"))]
    pub steps: Option<i32>,

    /// RNG seed (`-1` = random, default `42`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Sampling method (`"euler"`, `"euler_a"`, `"lcm"`, etc., `"auto"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "sample_method must not be empty"
    ))]
    pub sample_method: Option<String>,

    /// Sigma schedule (`"discrete"`, `"karras"`, etc., `"auto"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "scheduler must not be empty"
    ))]
    pub scheduler: Option<String>,

    /// CLIP skip layers (default `0` = auto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0, message = "clip_skip must be >= 0"))]
    pub clip_skip: Option<i32>,

    /// DDIM eta (default `0.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, message = "eta must be >= 0.0"))]
    pub eta: Option<f32>,

    /// Init-image influence strength for img2img (0鈥?, default `0.75`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0, message = "strength must be between 0.0 and 1.0"))]
    pub strength: Option<f32>,

    /// Init image as a base64-encoded data URI (`data:image/png;base64,...`).
    /// Required when `mode` is `img2img`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::api::validation::validate_non_blank",
        message = "init_image must not be empty"
    ))]
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

fn validate_image_generation_request(
    request: &ImageGenerationRequest,
) -> Result<(), ValidationError> {
    if request.prompt.len() > MAX_PROMPT_BYTES {
        let mut error = ValidationError::new("prompt_too_large");
        error.message = Some(
            format!(
                "prompt is too large ({} bytes); maximum is {} bytes",
                request.prompt.len(),
                MAX_PROMPT_BYTES
            )
            .into(),
        );
        return Err(error);
    }

    if request.n > MAX_IMAGES_PER_REQUEST {
        let mut error = ValidationError::new("invalid_n");
        error.message = Some(format!("n must be between 1 and {MAX_IMAGES_PER_REQUEST}").into());
        return Err(error);
    }

    if request.width > MAX_IMAGE_DIM || request.height > MAX_IMAGE_DIM {
        let mut error = ValidationError::new("invalid_dimensions");
        error.message = Some(format!("image dimensions must not exceed {MAX_IMAGE_DIM}").into());
        return Err(error);
    }

    if request.mode == ImageMode::Img2Img
        && request
            .init_image
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        let mut error = ValidationError::new("missing_init_image");
        error.message = Some("init_image is required for img2img mode".into());
        return Err(error);
    }

    Ok(())
}
