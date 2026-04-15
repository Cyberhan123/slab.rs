use crate::provider::HubProvider;

#[derive(Debug, Clone)]
pub struct DownloadProgressUpdate {
    pub provider: HubProvider,
    pub repo_id: String,
    pub filename: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub trait DownloadProgress: Send + Sync {
    fn on_start(&self, _update: &DownloadProgressUpdate) {}
    fn on_progress(&self, _update: &DownloadProgressUpdate) {}
    fn on_finish(&self, _update: &DownloadProgressUpdate) {}
}
