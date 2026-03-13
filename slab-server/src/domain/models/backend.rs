#[derive(Debug, Clone)]
pub struct BackendStatusQuery {
    pub backend_id: String,
}

#[derive(Debug, Clone)]
pub struct DownloadBackendLibCommand {
    pub backend_id: String,
    pub target_dir: String,
}

#[derive(Debug, Clone)]
pub struct ReloadBackendLibCommand {
    pub backend_id: String,
    pub lib_path: String,
    pub model_path: String,
    pub num_workers: u32,
}

#[derive(Debug, Clone)]
pub struct BackendStatusView {
    pub backend: String,
    pub status: String,
}
