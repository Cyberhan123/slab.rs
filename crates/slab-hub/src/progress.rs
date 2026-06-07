use std::sync::{Arc, Mutex};

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

#[derive(Clone)]
pub(crate) struct SharedDownloadProgress {
    observer: Arc<dyn DownloadProgress>,
    state: Arc<Mutex<DownloadProgressUpdate>>,
}

impl SharedDownloadProgress {
    pub(crate) fn new(
        provider: HubProvider,
        repo_id: &str,
        filename: &str,
        observer: Arc<dyn DownloadProgress>,
    ) -> Self {
        Self {
            observer,
            state: Arc::new(Mutex::new(DownloadProgressUpdate {
                provider,
                repo_id: repo_id.to_owned(),
                filename: filename.to_owned(),
                downloaded_bytes: 0,
                total_bytes: None,
            })),
        }
    }

    pub(crate) fn start(&self, total_bytes: Option<u64>) {
        let snapshot = self.update(|state| {
            state.total_bytes = total_bytes;
            state.downloaded_bytes = 0;
        });
        self.observer.on_start(&snapshot);
    }

    pub(crate) fn increment(&self, downloaded_bytes: u64) {
        let snapshot = self.update(|state| {
            state.downloaded_bytes += downloaded_bytes;
        });
        self.observer.on_progress(&snapshot);
    }

    pub(crate) fn progress(&self, total_bytes: Option<u64>, downloaded_bytes: u64) {
        let snapshot = self.update(|state| {
            state.total_bytes = total_bytes;
            state.downloaded_bytes = downloaded_bytes;
        });
        self.observer.on_progress(&snapshot);
    }

    pub(crate) fn finish(&self) {
        let snapshot = self.state.lock().expect("progress state").clone();
        self.observer.on_finish(&snapshot);
    }

    pub(crate) fn finish_with(&self, total_bytes: Option<u64>, downloaded_bytes: u64) {
        let snapshot = self.update(|state| {
            state.total_bytes = total_bytes;
            state.downloaded_bytes = downloaded_bytes;
        });
        self.observer.on_finish(&snapshot);
    }

    fn update(&self, update: impl FnOnce(&mut DownloadProgressUpdate)) -> DownloadProgressUpdate {
        let mut state = self.state.lock().expect("progress state");
        update(&mut state);
        state.clone()
    }
}
