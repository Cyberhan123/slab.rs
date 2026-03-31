use slab_types::RuntimeBackendId;
use slab_types::runtime::RuntimeModelReloadSpec;

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
pub struct ReloadBackendLibCommand {
    pub backend_id: RuntimeBackendId,
    pub spec: RuntimeModelReloadSpec,
    pub uses_legacy_flattened_load: bool,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}
