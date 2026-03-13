use std::sync::Arc;

use axum::extract::FromRef;

pub mod config;
pub mod model_state;
pub mod worker_state;

pub use config::AppConfig;
pub use model_state::ModelState;
pub use worker_state::{OperationManager, SubmitOperation, WorkerState};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub grpc: Arc<crate::infra::rpc::gateway::GrpcGateway>,
    pub store: Arc<crate::infra::db::AnyStore>,
    pub task_manager: Arc<OperationManager>,
    pub model_auto_unload: Arc<crate::model_auto_unload::ModelAutoUnloadManager>,
    pub model_state: Arc<ModelState>,
    pub worker_state: Arc<WorkerState>,
}

impl AppState {
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
            grpc,
            store,
            task_manager,
            model_auto_unload,
            model_state,
            worker_state,
        }
    }
}

impl FromRef<Arc<AppState>> for ModelState {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.model_state).clone()
    }
}

impl FromRef<Arc<AppState>> for WorkerState {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.worker_state).clone()
    }
}

impl FromRef<Arc<AppState>> for AppConfig {
    fn from_ref(input: &Arc<AppState>) -> Self {
        (*input.config).clone()
    }
}

