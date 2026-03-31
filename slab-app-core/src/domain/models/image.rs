use base64::Engine as _;

use crate::error::AppCoreError;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImageGenerationMode {
    Txt2Img,
    Img2Img,
}

#[derive(Debug, Clone)]
pub struct DecodedImageInput {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

#[derive(Debug, Clone)]
pub struct ImageGenerationCommand {
    pub model: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub n: u32,
    pub width: u32,
    pub height: u32,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub clip_skip: Option<i32>,
    pub eta: Option<f32>,
    pub strength: Option<f32>,
    pub init_image: Option<DecodedImageInput>,
    pub mode: ImageGenerationMode,
}

fn decode_init_image(data_uri: &str) -> Result<DecodedImageInput, AppCoreError> {
    let b64 = if let Some(pos) = data_uri.find("base64,") {
        &data_uri[pos + "base64,".len()..]
    } else {
        data_uri
    };

    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).map_err(|error| {
        AppCoreError::BadRequest(format!("init_image base64 decode failed: {error}"))
    })?;

    let image = image::load_from_memory(&bytes)
        .map_err(|error| AppCoreError::BadRequest(format!("init_image decode failed: {error}")))?;

    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    Ok(DecodedImageInput { data: rgb.into_raw(), width, height, channels: 3 })
}
