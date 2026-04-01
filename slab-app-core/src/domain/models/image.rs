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
