use serde::{Deserialize, Serialize};

use super::{TaskProgress, TaskStatus, TimedTextSegment};

pub const IMAGE_GENERATION_TASK_TYPE: &str = "image_generation";
pub const VIDEO_GENERATION_TASK_TYPE: &str = "video_generation";
pub const AUDIO_TRANSCRIPTION_TASK_TYPE: &str = "audio_transcription";

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
    pub request_data: serde_json::Value,
    pub result_data: Option<serde_json::Value>,
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
    pub request_data: serde_json::Value,
    pub result_data: Option<serde_json::Value>,
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
    pub vad_json: Option<serde_json::Value>,
    pub decode_json: Option<serde_json::Value>,
    pub transcript_text: Option<String>,
    pub segments: Option<Vec<TimedTextSegment>>,
    pub request_data: serde_json::Value,
    pub result_data: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}
