use std::sync::Arc;
use std::time::Duration;

pub mod config;
pub mod model_state;
pub mod worker_state;

use crate::domain::ports::RuntimeInferenceGateway;

pub use config::AppConfig;
pub use model_state::ModelState;
pub use worker_state::{OperationManager, SubmitOperation, WorkerState};

const SETTINGS_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

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
    ) -> Self {
        pmid.spawn_periodic_refresh(SETTINGS_REFRESH_INTERVAL);
        let task_manager = Arc::new(OperationManager::new());
        let runtime_gateway: Arc<dyn RuntimeInferenceGateway> =
            Arc::new(crate::infra::rpc::GrpcRuntimeInferenceGateway::new(Arc::clone(&grpc)));
        let model_auto_unload = Arc::new(crate::model_auto_unload::ModelAutoUnloadManager::new(
            Arc::clone(&pmid),
            Arc::clone(&runtime_gateway),
            Arc::clone(&runtime_status),
        ));
        let model_state = Arc::new(ModelState::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&store),
            Arc::clone(&grpc),
            Arc::clone(&runtime_gateway),
            Arc::clone(&runtime_status),
            Arc::clone(&model_auto_unload),
        ));
        let worker_state = Arc::new(WorkerState::new(
            Arc::clone(&config),
            Arc::clone(&store),
            Arc::clone(&grpc),
            runtime_gateway,
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
    ) -> Self {
        let context = Arc::new(AppContext::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&grpc),
            Arc::clone(&runtime_status),
            Arc::clone(&store),
        ));

        let agent = crate::infra::agent::bootstrap::build_agent_bootstrap(
            context.as_ref(),
            Arc::clone(&store),
        );

        let services = Arc::new(crate::domain::services::AppServices::new(
            (*context.model_state).clone(),
            (*context.worker_state).clone(),
            agent.service,
            agent.runtime,
            runtime_host,
        ));

        Self { context, services }
    }
}

#[cfg(feature = "axum")]
mod axum_extractors {
    use crate::context::{AppState, ModelState, WorkerState};
    use crate::domain::services::{
        AgentService, AudioService, BackendService, ChatService, FfmpegService, ImageService,
        ModelService, PluginService, SessionService, SettingsService, SetupService,
        SubtitleService, SystemService, TaskApplicationService, UiStateService, VideoService,
        WorkspaceLspService,
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

    impl FromRef<Arc<AppState>> for WorkspaceLspService {
        fn from_ref(input: &Arc<AppState>) -> Self {
            input.services.workspace_lsp.clone()
        }
    }
}
