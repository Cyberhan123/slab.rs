use crate::domain::models::{TaskProgress, TaskStatus};

#[derive(Debug, Clone)]
pub struct MediaTaskState {
    pub status: TaskStatus,
    pub progress: Option<TaskProgress>,
    pub error_msg: Option<String>,
    pub task_created_at: chrono::DateTime<chrono::Utc>,
    pub task_updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ImageGenerationTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub requested_count: u32,
    pub reference_image_path: Option<String>,
    pub primary_image_path: Option<String>,
    pub artifact_paths: Vec<String>,
    pub request_data: String,
    pub result_data: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ImageGenerationTaskViewRecord {
    pub task: ImageGenerationTaskRecord,
    pub state: MediaTaskState,
}

#[derive(Debug, Clone)]
pub struct NewImageGenerationTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub requested_count: u32,
    pub reference_image_path: Option<String>,
    pub request_data: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct VideoGenerationTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frames: i32,
    pub fps: f32,
    pub reference_image_path: Option<String>,
    pub video_path: Option<String>,
    pub request_data: String,
    pub result_data: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct VideoGenerationTaskViewRecord {
    pub task: VideoGenerationTaskRecord,
    pub state: MediaTaskState,
}

#[derive(Debug, Clone)]
pub struct NewVideoGenerationTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub model_path: String,
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub width: u32,
    pub height: u32,
    pub frames: i32,
    pub fps: f32,
    pub reference_image_path: Option<String>,
    pub request_data: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct AudioTranscriptionTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub source_path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad_json: Option<String>,
    pub decode_json: Option<String>,
    pub transcript_text: Option<String>,
    pub request_data: String,
    pub result_data: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct AudioTranscriptionTaskViewRecord {
    pub task: AudioTranscriptionTaskRecord,
    pub state: MediaTaskState,
}

#[derive(Debug, Clone)]
pub struct NewAudioTranscriptionTaskRecord {
    pub task_id: String,
    pub backend_id: String,
    pub model_id: Option<String>,
    pub source_path: String,
    pub language: Option<String>,
    pub prompt: Option<String>,
    pub detect_language: Option<bool>,
    pub vad_json: Option<String>,
    pub decode_json: Option<String>,
    pub request_data: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
