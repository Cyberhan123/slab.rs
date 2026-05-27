use std::sync::Arc;

pub mod config;
pub mod model_state;
pub mod worker_state;

use crate::domain::ports::RuntimeInferenceGateway;

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
    ) -> Self {
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

        // Build the AgentControl with port adapters and register built-in tools.
        let store_for_agent: Arc<dyn slab_agent::port::AgentStorePort> =
            Arc::clone(&store) as Arc<dyn slab_agent::port::AgentStorePort>;
        let sse_notify = Arc::new(crate::infra::sse_notify::SseNotifyAdapter::new());
        let agent_control =
            Arc::new(build_agent_control(&context, Arc::clone(&store), Arc::clone(&sse_notify)));
        let agent_service = crate::domain::services::AgentService::new(
            agent_control,
            store_for_agent,
            Arc::clone(&sse_notify),
        );

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
    notify: Arc<crate::infra::sse_notify::SseNotifyAdapter>,
) -> slab_agent::AgentControl {
    use slab_agent::{AgentControl, ToolRouter};
    use slab_agent_tools::ShellPolicy;
    use slab_sandboxing::{SandboxEnvironment, SandboxPolicy, create_platform_driver};

    let llm =
        Arc::new(crate::infra::agent_adapter::ServerLlmAdapter::new(Arc::clone(&ctx.model_state)));
    let store_adapter: Arc<dyn slab_agent::port::AgentStorePort> = store;
    let workspace_root =
        crate::domain::services::workspace_root_from_settings_path(&ctx.config.settings_path);
    let sandbox_driver = workspace_root.clone().and_then(|root| {
        let env = SandboxEnvironment::new(Some(root), SandboxPolicy::WorkspaceWrite);
        match create_platform_driver(env) {
            Ok(driver) => available_sandbox_driver(driver),
            Err(error) => {
                tracing::warn!(%error, "sandbox driver is unavailable; shell tool stays blocked");
                None
            }
        }
    });
    let shell_policy =
        if sandbox_driver.is_some() { ShellPolicy::Allow } else { ShellPolicy::Block };

    let mut tool_router = ToolRouter::new();
    let web_search_config = ctx.pmid.config().agent.tools.websearch;
    let mcp_client = build_agent_mcp_client(ctx);
    slab_agent_tools::register_all_tools(
        &mut tool_router,
        shell_policy,
        sandbox_driver,
        workspace_root,
        mcp_client,
        false,
        web_search_config,
    );

    let notify_port: Arc<dyn slab_agent::AgentNotifyPort> = notify.clone();
    let approval_port: Arc<dyn slab_agent::ApprovalPort> = notify;

    AgentControl::new(llm, store_adapter, notify_port, approval_port, Arc::new(tool_router), 32, 4)
}

fn build_agent_mcp_client(ctx: &AppContext) -> Option<Arc<slab_mcp::McpClient>> {
    if !ctx.pmid.config().agent.tools.mcp.enabled {
        return None;
    }

    tracing::warn!(
        "agent MCP tools are enabled, but no persisted MCP server launch config is wired yet"
    );
    Some(Arc::new(slab_mcp::McpClient::new()))
}

fn available_sandbox_driver(
    driver: Arc<dyn slab_sandboxing::SandboxDriver>,
) -> Option<Arc<dyn slab_sandboxing::SandboxDriver>> {
    let status = driver.setup_status();
    if !status.available {
        tracing::warn!(
            details = %status.details,
            "sandbox driver is unavailable; shell tool stays blocked"
        );
        return None;
    }
    if status.degraded {
        tracing::warn!(details = %status.details, "sandbox driver is degraded");
    }
    Some(driver)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use slab_sandboxing::{SandboxDriver, SandboxError, SandboxSetupStatus, SandboxedCommand};

    use super::available_sandbox_driver;

    struct StatusDriver {
        status: SandboxSetupStatus,
    }

    #[async_trait]
    impl SandboxDriver for StatusDriver {
        async fn run(
            &self,
            _cmd: SandboxedCommand,
        ) -> Result<slab_sandboxing::SandboxedOutput, SandboxError> {
            unreachable!("status tests do not execute the sandbox driver")
        }

        fn name(&self) -> &str {
            "status"
        }

        fn setup_status(&self) -> SandboxSetupStatus {
            self.status.clone()
        }
    }

    #[test]
    fn unavailable_sandbox_driver_is_rejected() {
        let driver = Arc::new(StatusDriver {
            status: SandboxSetupStatus::unavailable("missing sandbox runtime"),
        });

        assert!(available_sandbox_driver(driver).is_none());
    }

    #[test]
    fn degraded_available_sandbox_driver_is_allowed() {
        let driver =
            Arc::new(StatusDriver { status: SandboxSetupStatus::degraded("guard-only mode") });

        assert!(available_sandbox_driver(driver).is_some());
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
