use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use crate::domain::models::{
    DecodedVideoInitImage, VideoGenerationCommand,
    VideoGenerationRequestData as DomainVideoGenerationRequestData,
    VideoGenerationResultData as DomainVideoGenerationResultData, VideoGenerationTaskView,
};
use crate::error::AppCoreError;
use crate::schemas::tasks::{TaskProgressResponse, TaskStatus};

const MAX_PROMPT_BYTES: usize = 128 * 1024;

/// Request body for `POST /v1/video/generations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[validate(schema(function = "validate_video_generation_request"))]
pub struct VideoGenerationRequest {
    /// Optional catalog model identifier used for history attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "model_id must not be empty"
    ))]
    pub model_id: Option<String>,

    /// The model identifier to use.
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "model must not be empty"
    ))]
    pub model: String,

    /// Text description of the desired video content.
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "prompt must not be empty"
    ))]
    pub prompt: String,

    /// Negative text prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "negative_prompt must not be empty"
    ))]
    pub negative_prompt: Option<String>,

    /// Frame width in pixels (default `512`).
    #[serde(default = "default_width")]
    #[validate(range(min = 1, max = 2048, message = "width must be between 1 and 2048"))]
    pub width: u32,

    /// Frame height in pixels (default `512`).
    #[serde(default = "default_height")]
    #[validate(range(min = 1, max = 2048, message = "height must be between 1 and 2048"))]
    pub height: u32,

    /// Number of video frames to generate (default `16`).
    #[serde(default = "default_frames")]
    #[validate(range(min = 1, max = 120, message = "video_frames must be between 1 and 120"))]
    pub video_frames: i32,

    /// Output frames per second (default `8`).
    #[serde(default = "default_fps")]
    #[validate(range(min = 1.0, max = 60.0, message = "fps must be between 1 and 60"))]
    pub fps: f64,

    /// Classifier-Free Guidance scale (default `7.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, message = "cfg_scale must be >= 0.0"))]
    pub cfg_scale: Option<f32>,

    /// Distilled guidance (default `3.5`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, message = "guidance must be >= 0.0"))]
    pub guidance: Option<f32>,

    /// Number of denoising steps (default `20`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 1, message = "steps must be at least 1"))]
    pub steps: Option<i32>,

    /// RNG seed (default `42`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    /// Sampling method.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "sample_method must not be empty"
    ))]
    pub sample_method: Option<String>,

    /// Sigma schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "scheduler must not be empty"
    ))]
    pub scheduler: Option<String>,

    /// Init-image for video2video (base64 data URI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(custom(
        function = "crate::schemas::validation::validate_non_blank",
        message = "init_image must not be empty"
    ))]
    pub init_image: Option<String>,

    /// Strength for init-image influence (default `0.75`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[validate(range(min = 0.0, max = 1.0, message = "strength must be between 0.0 and 1.0"))]
    pub strength: Option<f32>,
}

fn default_width() -> u32 {
    512
}

fn default_height() -> u32 {
    512
}

fn default_frames() -> i32 {
    16
}

fn default_fps() -> f64 {
    8.0
}

fn validate_video_generation_request(
    request: &VideoGenerationRequest,
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

    if !request.fps.is_finite() {
        let mut error = ValidationError::new("invalid_fps");
        error.message = Some("fps must be a finite value".into());
        return Err(error);
    }

    Ok(())
}

impl TryFrom<VideoGenerationRequest> for VideoGenerationCommand {
    type Error = AppCoreError;

    fn try_from(request: VideoGenerationRequest) -> Result<Self, Self::Error> {
        let init_image = request.init_image.as_deref().map(decode_init_image).transpose()?;

        Ok(Self {
            model_id: request.model_id,
            model: request.model,
            prompt: request.prompt,
            negative_prompt: request.negative_prompt,
            width: request.width,
            height: request.height,
            video_frames: request.video_frames,
            fps: request.fps,
            cfg_scale: request.cfg_scale,
            guidance: request.guidance,
            steps: request.steps,
            seed: request.seed,
            sample_method: request.sample_method,
            scheduler: request.scheduler,
            init_image,
            strength: request.strength,
        })
    }
}

fn decode_init_image(data_uri: &str) -> Result<DecodedVideoInitImage, AppCoreError> {
    let (data, width, height) = crate::schemas::decode_base64_init_image(data_uri)?;
    Ok(DecodedVideoInitImage { data, width, height, channels: 3 })
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VideoGenerationRequestData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    pub model: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub video_frames: i32,
    pub fps: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VideoGenerationResultData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct VideoGenerationTaskResponse {
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
    pub model_path: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frames: i32,
    pub fps: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
    pub request_data: VideoGenerationRequestData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_data: Option<VideoGenerationResultData>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<VideoGenerationTaskView> for VideoGenerationTaskResponse {
    fn from(value: VideoGenerationTaskView) -> Self {
        Self {
            task_id: value.task_id,
            task_type: value.task_type,
            status: value.status.into(),
            progress: value.progress.map(Into::into),
            error_msg: value.error_msg,
            backend_id: value.backend_id,
            model_id: value.model_id,
            model_path: value.model_path,
            prompt: value.prompt,
            negative_prompt: value.negative_prompt,
            width: value.width,
            height: value.height,
            frames: value.frames,
            fps: value.fps,
            reference_image_url: value.reference_image_url,
            video_url: value.video_url,
            request_data: value.request_data.into(),
            result_data: value.result_data.map(Into::into),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<DomainVideoGenerationRequestData> for VideoGenerationRequestData {
    fn from(value: DomainVideoGenerationRequestData) -> Self {
        Self {
            model_id: value.model_id,
            model: value.model,
            prompt: value.prompt,
            negative_prompt: value.negative_prompt,
            width: value.width,
            height: value.height,
            video_frames: value.video_frames,
            fps: value.fps,
            cfg_scale: value.cfg_scale,
            guidance: value.guidance,
            steps: value.steps,
            seed: value.seed,
            sample_method: value.sample_method,
            scheduler: value.scheduler,
            strength: value.strength,
            reference_image_path: value.reference_image_path,
        }
    }
}

impl From<DomainVideoGenerationResultData> for VideoGenerationResultData {
    fn from(value: DomainVideoGenerationResultData) -> Self {
        Self { video_path: value.video_path }
    }
}
