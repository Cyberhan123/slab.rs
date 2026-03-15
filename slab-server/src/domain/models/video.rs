use base64::Engine as _;

use crate::api::v1::video::schema::VideoGenerationRequest;
use crate::error::ServerError;

#[derive(Debug, Clone)]
pub struct DecodedVideoInitImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

#[derive(Debug, Clone)]
pub struct VideoGenerationCommand {
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub video_frames: i32,
    pub fps: f32,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub init_image: Option<DecodedVideoInitImage>,
    pub strength: Option<f32>,
}

impl TryFrom<VideoGenerationRequest> for VideoGenerationCommand {
    type Error = ServerError;

    fn try_from(request: VideoGenerationRequest) -> Result<Self, Self::Error> {
        let init_image = request
            .init_image
            .as_deref()
            .map(decode_init_image)
            .transpose()?;

        Ok(Self {
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

fn decode_init_image(data_uri: &str) -> Result<DecodedVideoInitImage, ServerError> {
    let b64 = if let Some(pos) = data_uri.find("base64,") {
        &data_uri[pos + "base64,".len()..]
    } else {
        data_uri
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|error| {
            ServerError::BadRequest(format!("init_image base64 decode failed: {error}"))
        })?;
    let image = image::load_from_memory(&bytes)
        .map_err(|error| ServerError::BadRequest(format!("init_image decode failed: {error}")))?;
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok(DecodedVideoInitImage {
        data: rgb.into_raw(),
        width,
        height,
        channels: 3,
    })
}
