use std::sync::Arc;

use axum::extract::FromRef;

pub mod config;
pub mod model_state;
pub mod worker_state;

pub use config::AppConfig;
pub use model_state::ModelState;
pub use worker_state::{OperationManager, SubmitOperation, WorkerState};

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub model_state: Arc<ModelState>,
    pub worker_state: Arc<WorkerState>,
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let task_manager = Arc::new(OperationManager::new());
        let model_state = Arc::new(ModelState::new(
            Arc::clone(&config),
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&model_auto_unload),
        ));
        let worker_state = Arc::new(WorkerState::new(
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&model_auto_unload),
            Arc::clone(&task_manager),
        ));

        Self {
            config,
            model_state,
            worker_state,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub context: Arc<AppContext>,
    pub services: Arc<crate::services::AppServices>,
}

impl AppState {
    pub fn new(
        config: Arc<AppConfig>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let context = Arc::new(AppContext::new(config, grpc, store, model_auto_unload));
        let services = Arc::new(crate::services::AppServices::new(
            (*context.model_state).clone(),
            (*context.worker_state).clone(),
        ));

        Self { context, services }
    }
}

impl FromRef<Arc<AppState>> for ModelState {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.context.model_state).clone()
    }
}

impl FromRef<Arc<AppState>> for WorkerState {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.context.worker_state).clone()
    }
}

impl FromRef<Arc<AppState>> for AppConfig {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.context.config).clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::audio::AudioService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.audio.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::backend::BackendService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.backend.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::chat::ChatService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.chat.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::config::ConfigService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.config.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::ffmpeg::FfmpegService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.ffmpeg.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::images::ImagesService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.images.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::models::ModelsService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.models.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::session::SessionService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.session.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::system::SystemService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.system.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::tasks::TasksService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.tasks.clone()
    }
}

impl FromRef<Arc<AppState>> for crate::services::video::VideoService {
    fn from_ref(input: &Arc<AppState>) -> Self {
        input.services.video.clone()
    }
}
