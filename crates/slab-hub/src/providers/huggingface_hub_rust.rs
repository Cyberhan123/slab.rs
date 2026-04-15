use std::path::PathBuf;

use crate::client::HubClient;
use crate::error::{HubError, HubErrorKind, map_huggingface_hub_rust_error};
use crate::provider::HubProvider;

impl HubClient {
    pub(crate) async fn list_repo_files_with_huggingface_hub_rust(
        &self,
        repo_id: &str,
    ) -> Result<Vec<String>, HubError> {
        use huggingface_hub::RepoListFilesParams;

        let client = huggingface_hub::HFClient::new().map_err(|error| {
            HubError::operation(
                HubProvider::HuggingfaceHubRust,
                format!("failed to initialize huggingface-hub-rust client: {error}"),
            )
        })?;
        let repo = self.huggingface_hub_rust_repo(&client, repo_id)?;
        repo.list_files(&RepoListFilesParams::default()).await.map_err(|error| {
            map_huggingface_hub_rust_error(
                HubProvider::HuggingfaceHubRust,
                "list repo files failed",
                error,
            )
        })
    }

    pub(crate) async fn download_file_with_huggingface_hub_rust(
        &self,
        repo_id: &str,
        filename: &str,
    ) -> Result<PathBuf, HubError> {
        use huggingface_hub::RepoDownloadFileParams;

        let client = huggingface_hub::HFClient::new().map_err(|error| {
            HubError::operation(
                HubProvider::HuggingfaceHubRust,
                format!("failed to initialize huggingface-hub-rust client: {error}"),
            )
        })?;
        let repo = self.huggingface_hub_rust_repo(&client, repo_id)?;
        let local_dir = self
            .cache_dir
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join("huggingface-hub-rust")
            .join(repo_id.replace('/', "--"));

        repo.download_file(
            &RepoDownloadFileParams::builder().filename(filename).local_dir(local_dir).build(),
        )
        .await
        .map_err(|error| {
            map_huggingface_hub_rust_error(
                HubProvider::HuggingfaceHubRust,
                format!("download failed for {filename}"),
                error,
            )
        })
    }

    fn huggingface_hub_rust_repo(
        &self,
        client: &huggingface_hub::HFClient,
        repo_id: &str,
    ) -> Result<huggingface_hub::HFRepo, HubError> {
        let (owner, name) = split_repo_id(repo_id).ok_or_else(|| {
            HubError::new(
                HubErrorKind::InvalidRepoId,
                Some(HubProvider::HuggingfaceHubRust),
                format!("repo_id '{repo_id}' must contain a repository name"),
            )
        })?;
        Ok(client.model(owner, name))
    }
}

fn split_repo_id(repo_id: &str) -> Option<(&str, &str)> {
    let repo_id = repo_id.trim();
    if repo_id.is_empty() {
        return None;
    }
    match repo_id.split_once('/') {
        Some((owner, name)) if !name.trim().is_empty() => Some((owner.trim(), name.trim())),
        Some(_) => None,
        None => Some(("", repo_id)),
    }
}
