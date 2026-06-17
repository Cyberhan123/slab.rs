pub mod agent;
mod audio;
mod backend;
mod chat;
mod ffmpeg;
mod image;
mod model;
mod plugin;
mod pmid;
mod session;
mod settings;
pub(crate) mod setup;
mod subtitle;
mod system;
mod task;
mod ui_state;
mod video;
mod workspace;

pub use agent::AgentService;
pub use audio::AudioService;
pub use backend::BackendService;
pub use chat::ChatService;
pub use ffmpeg::FfmpegService;
pub use image::ImageService;
pub use model::ModelService;
pub use plugin::PluginService;
pub use pmid::PmidService;
pub use session::SessionService;
pub use settings::SettingsService;
pub use setup::SetupService;
pub use subtitle::SubtitleService;
pub use system::SystemService;
pub use task::TaskApplicationService;
pub use ui_state::UiStateService;
pub use video::VideoService;
pub(crate) use workspace::workspace_root_from_config;
pub use workspace::{WorkspaceLspService, WorkspaceService};

use std::sync::Arc;

use crate::context::{ModelState, WorkerState};
use crate::infra::agent::runtime::AgentRuntimeReloader;
use crate::infra::runtime::ManagedRuntimeHost;

#[derive(Clone)]
pub struct AppServices {
    pub audio: AudioService,
    pub backend: BackendService,
    pub chat: ChatService,
    pub ffmpeg: FfmpegService,
    pub image: ImageService,
    pub model: ModelService,
    pub settings: SettingsService,
    pub plugin: PluginService,
    pub session: SessionService,
    pub setup: SetupService,
    pub subtitle: SubtitleService,
    pub system: SystemService,
    pub task_application: TaskApplicationService,
    pub ui_state: UiStateService,
    pub video: VideoService,
    pub agent: AgentService,
    pub workspace_lsp: WorkspaceLspService,
}

impl AppServices {
    pub(crate) fn new(
        model_state: ModelState,
        worker_state: WorkerState,
        agent: AgentService,
        agent_runtime: AgentRuntimeReloader,
        runtime_host: Option<Arc<ManagedRuntimeHost>>,
    ) -> Self {
        let model = ModelService::new(model_state.clone(), worker_state.clone());
        Self {
            audio: AudioService::new(worker_state.clone()),
            backend: BackendService::new(model_state.clone()),
            chat: ChatService::new(model_state.clone()),
            ffmpeg: FfmpegService::new(worker_state.clone()),
            image: ImageService::new(worker_state.clone()),
            model: model.clone(),
            plugin: PluginService::new_with_agent_runtime(
                model_state.clone(),
                Some(agent_runtime.clone()),
            ),
            settings: SettingsService::new_with_agent_runtime(
                model_state.clone(),
                Some(agent_runtime),
            ),
            session: SessionService::new(model_state.clone()),
            setup: SetupService::new(model_state.clone(), worker_state.clone(), runtime_host),
            subtitle: SubtitleService::new(),
            system: SystemService::new_with_model_state(model_state.clone()),
            task_application: TaskApplicationService::new(worker_state.clone(), model),
            ui_state: UiStateService::new(model_state.clone()),
            video: VideoService::new(worker_state),
            agent,
            workspace_lsp: WorkspaceLspService::new(
                Arc::clone(model_state.config()),
                PluginService::new(model_state),
            ),
        }
    }
}
