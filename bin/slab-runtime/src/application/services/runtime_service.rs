use std::collections::HashMap;
use std::sync::Arc;

use slab_types::RuntimeBackendLoadSpec;
use slab_types::runtime::RuntimeModelStatus;
use tokio::sync::RwLock;

use crate::domain::services::{BackendSession, ExecutionHub};
use crate::infra::config::EnabledBackends;

use super::{
    BackendKind, BackendSessionService, ModelLifecycleService, RuntimeApplicationError,
    RuntimeState, SharedRuntimeState,
};

#[derive(Clone)]
pub struct RuntimeApplication {
    session_service: BackendSessionService,
    model_lifecycle_service: ModelLifecycleService,
}

impl RuntimeApplication {
    pub fn new(execution: ExecutionHub, enabled_backends: EnabledBackends) -> Self {
        let state: SharedRuntimeState = Arc::new(RwLock::new(RuntimeState {
            execution,
            enabled_backends,
            sessions: HashMap::new(),
        }));

        Self {
            session_service: BackendSessionService::new(state.clone()),
            model_lifecycle_service: ModelLifecycleService::new(state),
        }
    }

    pub async fn session_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<BackendSession, RuntimeApplicationError> {
        self.session_service.session_for_backend(backend).await
    }

    pub async fn load_model_for_backend(
        &self,
        backend: BackendKind,
        load_spec: RuntimeBackendLoadSpec,
    ) -> Result<RuntimeModelStatus, RuntimeApplicationError> {
        self.model_lifecycle_service.load_model_for_backend(backend, load_spec).await
    }

    pub async fn unload_model_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<RuntimeModelStatus, RuntimeApplicationError> {
        self.model_lifecycle_service.unload_model_for_backend(backend).await
    }
}
