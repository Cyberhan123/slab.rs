pub mod audio;
pub mod backend;
pub mod chat;
pub mod config;
pub mod ffmpeg;
pub mod images;
pub mod models;
pub mod session;
pub mod system;
pub mod tasks;
pub mod video;

use crate::context::{ModelState, WorkerState};

#[derive(Clone)]
pub struct AppServices {
    pub audio: audio::AudioService,
    pub backend: backend::BackendService,
    pub chat: chat::ChatService,
    pub config: config::ConfigService,
    pub ffmpeg: ffmpeg::FfmpegService,
    pub images: images::ImagesService,
    pub models: models::ModelsService,
    pub session: session::SessionService,
    pub system: system::SystemService,
    pub tasks: tasks::TasksService,
    pub video: video::VideoService,
}

impl AppServices {
    pub fn new(model_state: ModelState, worker_state: WorkerState) -> Self {
        Self {
            audio: audio::AudioService::new(worker_state.clone()),
            backend: backend::BackendService::new(model_state.clone(), worker_state.clone()),
            chat: chat::ChatService::new(model_state.clone()),
            config: config::ConfigService::new(model_state.clone()),
            ffmpeg: ffmpeg::FfmpegService::new(worker_state.clone()),
            images: images::ImagesService::new(worker_state.clone()),
            models: models::ModelsService::new(model_state.clone(), worker_state.clone()),
            session: session::SessionService::new(model_state.clone()),
            system: system::SystemService::new(),
            tasks: tasks::TasksService::new(worker_state.clone()),
            video: video::VideoService::new(worker_state),
        }
    }
}
