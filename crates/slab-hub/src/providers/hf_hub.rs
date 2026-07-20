use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use hf_hub::progress::{DownloadEvent, ProgressEvent, ProgressHandler};
use hf_hub::{HFClient, HFClientBuilder, split_id};

use crate::client::HubClient;
use crate::error::{HubError, map_hf_hub_error};
use crate::progress::{DownloadProgress, SharedDownloadProgress};
use crate::provider::HubProvider;

impl HubClient {
    pub(crate) async fn list_repo_files_with_hf_hub(
        &self,
        repo_id: &str,
    ) -> Result<Vec<String>, HubError> {
        let repo = self.hf_hub_repo(HubProvider::HfHub, repo_id)?;
        let info = repo.info().send().await.map_err(|error| {
            map_hf_hub_error(HubProvider::HfHub, "list repo files failed", error)
        })?;
        Ok(info.siblings.unwrap_or_default().into_iter().map(|item| item.rfilename).collect())
    }

    pub(crate) async fn download_file_with_hf_hub(
        &self,
        repo_id: &str,
        filename: &str,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<PathBuf, HubError> {
        let repo = self.hf_hub_repo(HubProvider::HfHub, repo_id)?;

        // Fast path: serve from the local cache without any network request.
        if let Ok(cached) =
            repo.download_file().filename(filename.to_owned()).local_files_only(true).send().await
        {
            return Ok(cached);
        }

        let builder = repo.download_file().filename(filename.to_owned());
        let result = match progress {
            Some(progress) => {
                let adapter =
                    HfHubProgressAdapter::new(HubProvider::HfHub, repo_id, filename, progress);
                builder.progress(adapter).send().await
            }
            None => builder.send().await,
        };
        result.map_err(|error| {
            map_hf_hub_error(HubProvider::HfHub, format!("download failed for {filename}"), error)
        })
    }

    fn hf_hub_repo(
        &self,
        provider: HubProvider,
        repo_id: &str,
    ) -> Result<hf_hub::HFRepository<hf_hub::RepoTypeModel>, HubError> {
        let client = self.hf_hub_client(provider)?;
        let (owner, name) = split_id(repo_id);
        Ok(client.model(owner.to_owned(), name.to_owned()))
    }

    fn hf_hub_client(&self, provider: HubProvider) -> Result<HFClient, HubError> {
        let mut builder = HFClientBuilder::new().endpoint(self.endpoints.hf_endpoint.clone());
        if let Some(cache_dir) = self.cache_dir.clone() {
            builder = builder.cache_dir(cache_dir).cache_enabled(true);
        }
        builder.build().map_err(|error| {
            map_hf_hub_error(provider, "failed to initialize hf-hub client", error)
        })
    }
}

struct HfHubProgressAdapter {
    progress: SharedDownloadProgress,
    /// Running total of bytes reported by hf-hub, used to convert the cumulative
    /// `DownloadEvent::Progress` byte counts into the delta increments expected by
    /// [`SharedDownloadProgress`].
    seen: AtomicU64,
}

impl HfHubProgressAdapter {
    fn new(
        provider: HubProvider,
        repo_id: &str,
        filename: &str,
        observer: Arc<dyn DownloadProgress>,
    ) -> Self {
        Self {
            progress: SharedDownloadProgress::new(provider, repo_id, filename, observer),
            seen: AtomicU64::new(0),
        }
    }
}

impl ProgressHandler for HfHubProgressAdapter {
    fn on_progress(&self, event: &ProgressEvent) {
        let ProgressEvent::Download(download) = event else { return };
        match download {
            DownloadEvent::Start { total_bytes, .. } => {
                self.progress.start(Some(*total_bytes));
            }
            DownloadEvent::Progress { files } => {
                let total: u64 = files.iter().map(|file| file.bytes_completed).sum();
                let previous = self.seen.swap(total, Ordering::Relaxed);
                if total > previous {
                    self.progress.increment(total - previous);
                }
            }
            DownloadEvent::Complete => {
                self.progress.finish();
            }
            _ => {}
        }
    }
}
