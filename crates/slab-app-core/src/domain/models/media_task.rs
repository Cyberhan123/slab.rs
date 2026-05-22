use serde::{Deserialize, Serialize};

use super::{TaskProgress, TaskStatus, TimedTextSegment};
use crate::domain::models::{TranscribeDecodeOptions, TranscribeVadOptions};

pub const IMAGE_GENERATION_TASK_TYPE: &str = "image_generation";
pub const VIDEO_GENERATION_TASK_TYPE: &str = "video_generation";
pub const AUDIO_TRANSCRIPTION_TASK_TYPE: &str = "audio_transcription";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageGenerationRequestData {
    pub model_id: Option<String>,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub n: u32,
    pub width: u32,
    pub height: u32,
    pub model: String,
    pub mode: String,
    pub cfg_scale: Option<f32>,
    pub guidance: Option<f32>,
    pub steps: Option<i32>,
    pub seed: Option<i64>,
    pub sample_method: Option<String>,
    pub scheduler: Option<String>,
    pub clip_skip: Option<i32>,
    pub strength: Option<f32>,
    pub eta: Option<f32>,
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageGenerationResultData {
    pub primary_image_path: Option<String>,
    pub artifact_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoGenerationRequestData {
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
    pub strength: Option<f32>,
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoGenerationResultData {
    pub video_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioTranscriptionRequestData {
    pub model_id: Option<String>,
    pub source_path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad: Option<TranscribeVadOptions>,
    pub decode: Option<TranscribeDecodeOptions>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioTranscriptionResultData {
    pub text: String,
    pub segments: Vec<TimedTextSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationTaskView {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub requested_count: u32,
    pub reference_image_url: Option<String>,
    pub primary_image_url: Option<String>,
    pub image_urls: Vec<String>,
    pub request_data: ImageGenerationRequestData,
    pub result_data: Option<ImageGenerationResultData>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoGenerationTaskView {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frames: i32,
    pub fps: f32,
    pub reference_image_url: Option<String>,
    pub video_url: Option<String>,
    pub request_data: VideoGenerationRequestData,
    pub result_data: Option<VideoGenerationResultData>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTranscriptionTaskView {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub source_path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad_json: Option<TranscribeVadOptions>,
    pub decode_json: Option<TranscribeDecodeOptions>,
    pub transcript_text: Option<String>,
    pub segments: Option<Vec<TimedTextSegment>>,
    pub request_data: AudioTranscriptionRequestData,
    pub result_data: Option<AudioTranscriptionResultData>,
    pub created_at: String,
    pub updated_at: String,
}
