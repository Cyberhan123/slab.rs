use slab_runtime_core::CoreError;

use crate::domain::services::BackendSession;

use super::{BackendKind, RuntimeApplicationError, SharedRuntimeState};

#[derive(Clone)]
pub(crate) struct BackendSessionService {
    state: SharedRuntimeState,
}

impl BackendSessionService {
    pub(crate) fn new(state: SharedRuntimeState) -> Self {
        Self { state }
    }

    pub(crate) async fn session_for_backend(
        &self,
        backend: BackendKind,
    ) -> Result<BackendSession, RuntimeApplicationError> {
        let state = self.state.read().await;
        state.ensure_enabled(backend)?;
        state
            .sessions
            .get(&backend)
            .cloned()
            .ok_or(RuntimeApplicationError::Runtime(CoreError::ModelNotLoaded))
    }
}