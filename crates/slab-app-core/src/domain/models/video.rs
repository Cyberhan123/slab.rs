#[derive(Debug, Clone)]
pub struct DecodedVideoInitImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

#[derive(Debug, Clone)]
pub struct VideoGenerationCommand {
    pub model_id: Option<String>,
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
