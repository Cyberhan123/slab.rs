mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod model;
mod session;
mod settings;
mod system;
mod task;
mod video;

pub use audio::AudioService;
pub use backend::BackendService;
pub use chat::ChatService;
pub use ffmpeg::FfmpegService;
pub use image::ImageService;
pub use model::ModelService;
pub use session::SessionService;
pub use settings::SettingsService;
pub use system::SystemService;
pub use task::TaskApplicationService;
pub use video::VideoService;

use crate::context::{ModelState, WorkerState};

#[derive(Clone)]
pub struct AppServices {
    pub audio: AudioService,
    pub backend: BackendService,
    pub chat: ChatService,
    pub ffmpeg: FfmpegService,
    pub image: ImageService,
    pub model: ModelService,
    pub settings: SettingsService,
    pub session: SessionService,
    pub system: SystemService,
    pub task_application: TaskApplicationService,
    pub video: VideoService,
}

impl AppServices {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self {
            audio: AudioService::new(worker_state.clone()),
            backend: BackendService::new(model_state.clone(), worker_state.clone()),
            chat: ChatService::new(model_state.clone()),
            ffmpeg: FfmpegService::new(worker_state.clone()),
            image: ImageService::new(worker_state.clone()),
            model: ModelService::new(model_state.clone(), worker_state.clone()),
            settings: SettingsService::new(model_state.clone()),
            session: SessionService::new(model_state.clone()),
            system: SystemService::new(),
            task_application: TaskApplicationService::new(worker_state.clone()),
            video: VideoService::new(worker_state),
        }
    }
}
