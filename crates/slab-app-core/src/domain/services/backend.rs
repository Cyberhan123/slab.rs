use slab_types::RuntimeBackendId;

use crate::context::ModelState;
use crate::domain::models::{BackendStatusQuery, BackendStatusView};
use crate::error::AppCoreError;
use crate::runtime_supervisor::RuntimeBackendRuntimeStatus;

#[derive(Clone)]
pub struct BackendService {
    model_state: ModelState,
}

impl BackendService {
    pub fn new(model_state: ModelState) -> Self {
        Self { model_state }
    }

    pub async fn backend_status(
        &self,
        query: BackendStatusQuery,
    ) -> Result<BackendStatusView, AppCoreError> {
        let canonical_backend = query.backend_id.to_string();
        Ok(BackendStatusView {
            backend: canonical_backend,
            status: runtime_status_label(
                self.model_state.runtime_status().status(query.backend_id),
            )
            .to_owned(),
        })
    }

    pub async fn list_backends(&self) -> Result<Vec<BackendStatusView>, AppCoreError> {
        let backends = RuntimeBackendId::ALL
            .into_iter()
            .map(|name| BackendStatusView {
                backend: name.to_string(),
                status: runtime_status_label(self.model_state.runtime_status().status(name))
                    .to_owned(),
            })
            .collect();
        Ok(backends)
    }
}

fn runtime_status_label(status: RuntimeBackendRuntimeStatus) -> &'static str {
    status.as_str()
}

#[cfg(test)]
mod tests {
    use super::runtime_status_label;
    use crate::runtime_supervisor::RuntimeBackendRuntimeStatus;

    #[test]
    fn runtime_status_labels_match_backend_api_surface() {
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Ready), "ready");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Restarting), "restarting");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Unavailable), "unavailable");
        assert_eq!(runtime_status_label(RuntimeBackendRuntimeStatus::Disabled), "disabled");
    }
}
