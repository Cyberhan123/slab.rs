use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::client::HubClient;
use crate::error::{HubError, map_models_cat_error};
use crate::progress::{DownloadProgress, DownloadProgressUpdate};
use crate::provider::HubProvider;

impl HubClient {
    pub(crate) async fn list_repo_files_with_models_cat(
        &self,
        repo_id: &str,
    ) -> Result<Vec<String>, HubError> {
        let repo = self.models_cat_repo(repo_id);
        let client = models_cat::asynchronous::ModelsCat::new_with_endpoint(
            repo,
            self.endpoints.models_cat_endpoint.clone(),
        );
        client.list_hub_files().await.map_err(|error| {
            map_models_cat_error(HubProvider::ModelsCat, "list repo files failed", error)
        })
    }

    pub(crate) async fn download_file_with_models_cat(
        &self,
        repo_id: &str,
        filename: &str,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<PathBuf, HubError> {
        let repo = self.models_cat_repo(repo_id);
        let client = models_cat::asynchronous::ModelsCat::new_with_endpoint(
            repo.clone(),
            self.endpoints.models_cat_endpoint.clone(),
        );
        match progress {
            Some(progress) => {
                let adapter = ModelsCatProgressAdapter::new(
                    HubProvider::ModelsCat,
                    repo_id,
                    filename,
                    progress,
                );
                client.download_with_progress(filename, adapter).await.map_err(|error| {
                    map_models_cat_error(
                        HubProvider::ModelsCat,
                        format!("download failed for {filename}"),
                        error,
                    )
                })?;
            }
            None => {
                client.download(filename).await.map_err(|error| {
                    map_models_cat_error(
                        HubProvider::ModelsCat,
                        format!("download failed for {filename}"),
                        error,
                    )
                })?;
            }
        }

        find_models_cat_downloaded_path(&repo, filename)?.ok_or_else(|| {
            HubError::operation(
                HubProvider::ModelsCat,
                format!("download succeeded for {filename} but no local file was found"),
            )
        })
    }

    fn models_cat_repo(&self, repo_id: &str) -> models_cat::Repo {
        let mut repo = models_cat::Repo::new_model(repo_id);
        if let Some(cache_dir) = self.cache_dir.as_ref() {
            let models_cat_cache_dir: PathBuf = cache_dir.join("models-cat");
            repo.set_cache_dir(models_cat_cache_dir);
        }
        repo
    }
}

fn find_models_cat_downloaded_path(
    repo: &models_cat::Repo,
    filename: &str,
) -> Result<Option<PathBuf>, HubError> {
    let base_path = repo.cache_dir().join("snapshots");
    let target = Path::new(filename);
    let mut snapshot_dirs = Vec::new();

    for entry in std::fs::read_dir(&base_path).map_err(|error| {
        HubError::operation(
            HubProvider::ModelsCat,
            format!(
                "failed to read models-cat snapshot directory '{}': {error}",
                base_path.display()
            ),
        )
    })? {
        let entry = entry.map_err(|error| {
            HubError::operation(
                HubProvider::ModelsCat,
                format!(
                    "failed to iterate models-cat snapshot entries under '{}': {error}",
                    base_path.display()
                ),
            )
        })?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| {
            HubError::operation(
                HubProvider::ModelsCat,
                format!(
                    "failed to inspect models-cat snapshot metadata under '{}': {error}",
                    base_path.display()
                ),
            )
        })?;

        if !metadata.is_dir() {
            continue;
        }

        let modified = metadata.modified().map_err(|error| {
            HubError::operation(
                HubProvider::ModelsCat,
                format!(
                    "failed to read models-cat snapshot timestamp under '{}': {error}",
                    base_path.display()
                ),
            )
        })?;
        snapshot_dirs.push((modified, path));
    }

    snapshot_dirs.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(snapshot_dirs.into_iter().find_map(|(_, snapshot_dir)| {
        let candidate = snapshot_dir.join(target);
        candidate.is_file().then_some(candidate)
    }))
}

#[derive(Clone)]
struct ModelsCatProgressAdapter {
    observer: Arc<dyn DownloadProgress>,
    state: Arc<Mutex<DownloadProgressUpdate>>,
}

impl ModelsCatProgressAdapter {
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

#[async_trait::async_trait]
impl models_cat::asynchronous::Progress for ModelsCatProgressAdapter {
    async fn on_start(
        &mut self,
        unit: &models_cat::asynchronous::ProgressUnit,
    ) -> Result<(), models_cat::OpsError> {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state");
            state.total_bytes = Some(unit.total_size());
            state.downloaded_bytes = 0;
            state.clone()
        };
        self.observer.on_start(&snapshot);
        Ok(())
    }

    async fn on_progress(
        &mut self,
        unit: &models_cat::asynchronous::ProgressUnit,
    ) -> Result<(), models_cat::OpsError> {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state");
            state.total_bytes = Some(unit.total_size());
            state.downloaded_bytes = unit.current();
            state.clone()
        };
        self.observer.on_progress(&snapshot);
        Ok(())
    }

    async fn on_finish(
        &mut self,
        unit: &models_cat::asynchronous::ProgressUnit,
    ) -> Result<(), models_cat::OpsError> {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state");
            state.total_bytes = Some(unit.total_size());
            state.downloaded_bytes = unit.current();
            state.clone()
        };
        self.observer.on_finish(&snapshot);
        Ok(())
    }
}
