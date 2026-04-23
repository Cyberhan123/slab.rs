use std::sync::Arc;

pub mod config;
pub mod model_state;
pub mod worker_state;

pub use config::AppConfig;
pub use model_state::ModelState;
pub use worker_state::{OperationManager, SubmitOperation, WorkerState};

#[derive(Clone)]
pub struct AppContext {
    pub config: Arc<AppConfig>,
    pub pmid: Arc<crate::domain::services::PmidService>,
    pub model_state: Arc<ModelState>,
    pub worker_state: Arc<WorkerState>,
}

impl AppContext {
    pub fn new(
        config: Arc<AppConfig>,
        pmid: Arc<crate::domain::services::PmidService>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let task_manager = Arc::new(OperationManager::new());
        let model_state = Arc::new(ModelState::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&runtime_status),
            Arc::clone(&model_auto_unload),
        ));
        let worker_state = Arc::new(WorkerState::new(
            Arc::clone(&config),
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&runtime_status),
            Arc::clone(&model_auto_unload),
            Arc::clone(&task_manager),
        ));

        Self { config, pmid, model_state, worker_state }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub context: Arc<AppContext>,
    pub services: Arc<crate::domain::services::AppServices>,
}

impl AppState {
    pub fn new(
        config: Arc<AppConfig>,
        pmid: Arc<crate::domain::services::PmidService>,
        grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
        runtime_status: Arc<crate::runtime_supervisor::RuntimeSupervisorStatus>,
        runtime_host: Option<Arc<crate::infra::runtime::ManagedRuntimeHost>>,
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let context = Arc::new(AppContext::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&grpc),
            Arc::clone(&runtime_status),
            Arc::clone(&store),
            Arc::clone(&model_auto_unload),
        ));

        // Build the AgentControl with port adapters and register built-in tools.
        let store_for_agent: Arc<dyn slab_agent::port::AgentStorePort> =
            Arc::clone(&store) as Arc<dyn slab_agent::port::AgentStorePort>;
        let agent_control = Arc::new(build_agent_control(&context, Arc::clone(&store)));
        let agent_service =
            crate::domain::services::AgentService::new(agent_control, store_for_agent);

        let services = Arc::new(crate::domain::services::AppServices::new(
            (*context.model_state).clone(),
            (*context.worker_state).clone(),
            agent_service,
            runtime_host,
        ));

        Self { context, services }
    }
}

/// Construct the [`slab_agent::AgentControl`] singleton, wiring up the port
/// adapters and registering built-in tools.
fn build_agent_control(
    ctx: &AppContext,
    store: Arc<crate::infra::db::AnyStore>,
) -> slab_agent::AgentControl {
    use crate::infra::agent_adapter::{NoopNotifyAdapter, ServerLlmAdapter};
    use slab_agent::{AgentControl, ToolRouter};

    let llm = Arc::new(ServerLlmAdapter::new(Arc::clone(&ctx.model_state)));
    let store_adapter: Arc<dyn slab_agent::port::AgentStorePort> = store;
    let notify = Arc::new(NoopNotifyAdapter);

    let mut tool_router = ToolRouter::new();
    slab_agent_tools::register_builtin_tools(&mut tool_router);

    AgentControl::new(llm, store_adapter, notify, Arc::new(tool_router), 32, 4)
}

#[cfg(feature = "axum")]
mod axum_extractors {
    use crate::context::{AppConfig, AppState, ModelState, WorkerState};
    use crate::domain::services::{
        AgentService, AudioService, BackendService, ChatService, FfmpegService, ImageService,
        ModelService, PluginService, PmidService, SessionService, SettingsService, SetupService,
        SubtitleService, SystemService, TaskApplicationService, UiStateService, VideoService,
    };
    use axum::extract::FromRef;
    use std::sync::Arc;

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

    impl FromRef<Arc<AppState>> for PmidService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            (*input.context.pmid).clone()
        }
    }

    impl FromRef<Arc<AppState>> for AudioService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.audio.clone()
        }
    }

    impl FromRef<Arc<AppState>> for BackendService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.backend.clone()
        }
    }

    impl FromRef<Arc<AppState>> for ChatService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.chat.clone()
        }
    }

    impl FromRef<Arc<AppState>> for FfmpegService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.ffmpeg.clone()
        }
    }

    impl FromRef<Arc<AppState>> for ImageService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.image.clone()
        }
    }

    impl FromRef<Arc<AppState>> for ModelService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.model.clone()
        }
    }

    impl FromRef<Arc<AppState>> for SettingsService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.settings.clone()
        }
    }

    impl FromRef<Arc<AppState>> for PluginService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.plugin.clone()
        }
    }

    impl FromRef<Arc<AppState>> for SessionService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.session.clone()
        }
    }

    impl FromRef<Arc<AppState>> for SystemService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.system.clone()
        }
    }

    impl FromRef<Arc<AppState>> for SubtitleService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.subtitle.clone()
        }
    }

    impl FromRef<Arc<AppState>> for TaskApplicationService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.task_application.clone()
        }
    }

    impl FromRef<Arc<AppState>> for UiStateService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.ui_state.clone()
        }
    }

    impl FromRef<Arc<AppState>> for VideoService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.video.clone()
        }
    }

    impl FromRef<Arc<AppState>> for SetupService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.setup.clone()
        }
    }

    impl FromRef<Arc<AppState>> for AgentService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.agent.clone()
        }
    }
}
