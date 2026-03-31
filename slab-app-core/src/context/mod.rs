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
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let task_manager = Arc::new(OperationManager::new());
        let model_state = Arc::new(ModelState::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
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
        store: Arc<crate::infra::db::AnyStore>,
        model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    ) -> Self {
        let context = Arc::new(AppContext::new(
            Arc::clone(&config),
            Arc::clone(&pmid),
            Arc::clone(&grpc),
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

    // Register built-in tools.
    let mut tool_router = ToolRouter::new();
    tool_router.register(Box::new(slab_agent::tools::EchoTool));

    AgentControl::new(llm, store_adapter, notify, Arc::new(tool_router), 32, 4)
}
