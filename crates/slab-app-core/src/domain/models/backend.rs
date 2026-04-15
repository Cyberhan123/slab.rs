use slab_types::RuntimeBackendId;

#[derive(Debug, Clone)]
pub struct BackendStatusQuery {
    pub backend_id: RuntimeBackendId,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}
