pub mod agent;
mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod model;
mod pmid;
mod session;
mod settings;
mod setup;
mod system;
mod task;
mod video;

pub use agent::AgentService;
pub use audio::AudioService;
pub use backend::BackendService;
pub use chat::ChatService;
pub use ffmpeg::FfmpegService;
pub use image::ImageService;
pub use model::ModelService;
pub use pmid::PmidService;
pub use session::SessionService;
pub use settings::SettingsService;
pub use setup::SetupService;
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
    pub setup: SetupService,
    pub system: SystemService,
    pub task_application: TaskApplicationService,
    pub video: VideoService,
    pub agent: AgentService,
}

impl AppServices {
    pub fn new(model_state: ModelState, worker_state: WorkerState, agent: AgentService) -> Self {
        Self {
            audio: AudioService::new(worker_state.clone()),
            backend: BackendService::new(model_state.clone(), worker_state.clone()),
            chat: ChatService::new(model_state.clone()),
            ffmpeg: FfmpegService::new(worker_state.clone()),
            image: ImageService::new(worker_state.clone()),
            model: ModelService::new(model_state.clone(), worker_state.clone()),
            settings: SettingsService::new(model_state.clone()),
            session: SessionService::new(model_state.clone()),
            setup: SetupService::new(model_state.clone(), worker_state.clone()),
            system: SystemService::new(),
            task_application: TaskApplicationService::new(worker_state.clone()),
            video: VideoService::new(worker_state),
            agent,
        }
    }
}
