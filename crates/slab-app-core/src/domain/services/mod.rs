pub mod agent;
mod audio;
mod backend;
mod chat;
mod cloud_activation;
mod ffmpeg;
mod image;
pub(crate) mod llm;
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

pub use agent::{HarnessService, ResponseService, TurnTimelineEntry};
pub use audio::AudioService;
pub use backend::BackendService;
pub use chat::ChatService;
pub use ffmpeg::FfmpegService;
pub use image::ImageService;
pub(crate) use model::resolve_local_chat_prompt_profile;
pub use model::{ModelLoadProgress, ModelService};
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

/// Runtime workspace-switch surface for the HTTP layer: re-points the live
/// agent (sandbox-bound tool registrations + the thread context future
/// threads spawn with) at a newly opened/closed workspace root. Wraps the
/// infra reloader without exposing it outside app-core.
#[derive(Clone)]
pub struct WorkspaceAgentRuntime {
    reloader: AgentRuntimeReloader,
}

impl WorkspaceAgentRuntime {
    pub(crate) fn new(reloader: AgentRuntimeReloader) -> Self {
        Self { reloader }
    }

    /// Refresh the agent's workspace-bound state for `root` (`None` = closed:
    /// the workspace-bound tools are unregistered).
    pub fn refresh_workspace(&self, root: Option<std::path::PathBuf>) {
        self.reloader.refresh_workspace_tools(root);
    }

    /// Stop every background task whose output lives under `root` (workspace
    /// migration: no "ghost" dev servers carry into the new workspace).
    /// Returns the stopped task ids.
    pub fn stop_background_tasks_for(&self, root: &std::path::Path) -> Vec<String> {
        self.reloader.stop_background_tasks_for(root)
    }
}

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
    pub harness: HarnessService,
    pub response: ResponseService,
    pub workspace_lsp: WorkspaceLspService,
    pub workspace_agent: WorkspaceAgentRuntime,
}

impl AppServices {
    pub(crate) fn new(
        model_state: ModelState,
        worker_state: WorkerState,
        harness: HarnessService,
        response: ResponseService,
        agent_runtime: AgentRuntimeReloader,
        runtime_host: Option<Arc<ManagedRuntimeHost>>,
    ) -> Self {
        let model = ModelService::new(model_state.clone(), worker_state.clone());
        let workspace_agent = WorkspaceAgentRuntime::new(agent_runtime.clone());
        Self {
            audio: AudioService::new(worker_state.clone()),
            backend: BackendService::new(model_state.clone()),
            chat: ChatService::new_with_compact(model_state.clone(), harness.compact_port()),
            ffmpeg: FfmpegService::new(worker_state.clone()),
            image: ImageService::new(worker_state.clone()),
            model: model.clone(),
            plugin: PluginService::new_with_agent_runtime(
                model_state.clone(),
                Some(agent_runtime.clone()),
            ),
            settings: SettingsService::new_with(
                model_state.clone(),
                Some(agent_runtime),
                Some(model.clone()),
            ),
            session: SessionService::new(model_state.clone()),
            setup: SetupService::new(model_state.clone(), worker_state.clone(), runtime_host),
            subtitle: SubtitleService::new(),
            system: SystemService::new_with_model_state(model_state.clone()),
            task_application: TaskApplicationService::new(worker_state.clone(), model),
            ui_state: UiStateService::new(model_state.clone()),
            video: VideoService::new(worker_state),
            harness,
            response,
            workspace_lsp: WorkspaceLspService::new(
                Arc::clone(model_state.config()),
                PluginService::new(model_state),
            ),
            workspace_agent,
        }
    }
}
