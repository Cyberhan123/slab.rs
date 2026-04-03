use slab_types::RuntimeBackendId;

#[derive(Debug, Clone)]
pub struct BackendStatusQuery {
    pub backend_id: RuntimeBackendId,
}

#[derive(Debug, Clone)]
pub struct DownloadBackendLibCommand {
    pub backend_id: RuntimeBackendId,
    pub target_dir: String,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}
