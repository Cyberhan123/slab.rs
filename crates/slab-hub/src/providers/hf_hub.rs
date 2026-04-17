use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use hf_hub::{Cache, Repo, RepoType};

use crate::client::HubClient;
use crate::error::{HubError, map_hf_hub_error};
use crate::progress::{DownloadProgress, DownloadProgressUpdate};
use crate::provider::HubProvider;

impl HubClient {
    pub(crate) async fn list_repo_files_with_hf_hub(
        &self,
        repo_id: &str,
    ) -> Result<Vec<String>, HubError> {
        let api = self.hf_hub_api(HubProvider::HfHub)?;
        let repo = api.model(repo_id.to_owned());
        let info = repo.info().await.map_err(|error| {
            map_hf_hub_error(HubProvider::HfHub, "list repo files failed", error)
        })?;
        Ok(info.siblings.into_iter().map(|item| item.rfilename).collect())
    }

    pub(crate) async fn download_file_with_hf_hub(
        &self,
        repo_id: &str,
        filename: &str,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<PathBuf, HubError> {
        if let Some(cached_path) = self.hf_hub_cached_path(repo_id, filename) {
            return Ok(cached_path);
        }

        let api = self.hf_hub_api(HubProvider::HfHub)?;
        let repo = api.model(repo_id.to_owned());
        match progress {
            Some(progress) => {
                let adapter =
                    HfHubProgressAdapter::new(HubProvider::HfHub, repo_id, filename, progress);
                repo.download_with_progress(filename, adapter).await.map_err(|error| {
                    map_hf_hub_error(
                        HubProvider::HfHub,
                        format!("download failed for {filename}"),
                        error,
                    )
                })
            }
            None => repo.get(filename).await.map_err(|error| {
                map_hf_hub_error(
                    HubProvider::HfHub,
                    format!("download failed for {filename}"),
                    error,
                )
            }),
        }
    }

    fn hf_hub_api(&self, provider: HubProvider) -> Result<hf_hub::api::tokio::Api, HubError> {
        let mut builder = hf_hub::api::tokio::ApiBuilder::from_env()
            .with_progress(false)
            .with_endpoint(self.endpoints.hf_endpoint.clone());
        if let Some(cache_dir) = self.cache_dir.clone() {
            builder = builder.with_cache_dir(cache_dir);
        }
        builder.build().map_err(|error| {
            map_hf_hub_error(provider, "failed to initialize hf-hub client", error)
        })
    }

    fn hf_hub_cached_path(&self, repo_id: &str, filename: &str) -> Option<PathBuf> {
        let cache = self.cache_dir.clone().map(Cache::new).unwrap_or_else(Cache::from_env);
        cache.repo(Repo::new(repo_id.to_owned(), RepoType::Model)).get(filename)
    }
}

#[derive(Clone)]
struct HfHubProgressAdapter {
    observer: Arc<dyn DownloadProgress>,
    state: Arc<Mutex<DownloadProgressUpdate>>,
}

impl HfHubProgressAdapter {
    fn new(
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
}

impl hf_hub::api::tokio::Progress for HfHubProgressAdapter {
    async fn init(&mut self, size: usize, _filename: &str) {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state");
            state.total_bytes = Some(size as u64);
            state.downloaded_bytes = 0;
            state.clone()
        };
        self.observer.on_start(&snapshot);
    }

    async fn update(&mut self, size: usize) {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state");
            state.downloaded_bytes += size as u64;
            state.clone()
        };
        self.observer.on_progress(&snapshot);
    }

    async fn finish(&mut self) {
        let snapshot = self.state.lock().expect("progress state").clone();
        self.observer.on_finish(&snapshot);
    }
}
