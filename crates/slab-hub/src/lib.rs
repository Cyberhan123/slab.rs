use std::future::Future;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HubProvider {
    HfHub,
    ModelsCat,
    HuggingfaceHubRust,
}

impl HubProvider {
    pub fn as_config_value(self) -> &'static str {
        match self {
            Self::HfHub => "hf_hub",
            Self::ModelsCat => "models_cat",
            Self::HuggingfaceHubRust => "huggingface_hub_rust",
        }
    }

    fn base_url(self, endpoints: &HubEndpoints) -> &str {
        match self {
            Self::HfHub | Self::HuggingfaceHubRust => endpoints.hf_endpoint.as_str(),
            Self::ModelsCat => endpoints.models_cat_endpoint.as_str(),
        }
    }
}

impl std::fmt::Display for HubProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_config_value())
    }
}

impl FromStr for HubProvider {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "hf" | "hf_hub" | "huggingface" => Ok(Self::HfHub),
            "models_cat" | "modelscope" => Ok(Self::ModelsCat),
            "huggingface_hub_rust" | "huggingface_hub" => Ok(Self::HuggingfaceHubRust),
            other => Err(format!(
                "unsupported hub provider '{other}'; expected one of hf_hub, models_cat, huggingface_hub_rust"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HubProviderPreference {
    #[default]
    Auto,
    Provider(HubProvider),
}

impl HubProviderPreference {
    pub fn from_optional_str(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::Auto),
            Some("auto") => Ok(Self::Auto),
            Some(value) => HubProvider::from_str(value).map(Self::Provider),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubErrorKind {
    NetworkUnavailable,
    UnsupportedProvider,
    InvalidRepoId,
    OperationFailed,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct HubError {
    kind: HubErrorKind,
    provider: Option<HubProvider>,
    message: String,
}

impl HubError {
    pub fn kind(&self) -> HubErrorKind {
        self.kind
    }

    pub fn provider(&self) -> Option<HubProvider> {
        self.provider
    }

    fn new(kind: HubErrorKind, provider: Option<HubProvider>, message: impl Into<String>) -> Self {
        Self { kind, provider, message: message.into() }
    }

    fn operation(provider: HubProvider, message: impl Into<String>) -> Self {
        Self::new(HubErrorKind::OperationFailed, Some(provider), message)
    }
}

#[derive(Debug, Clone)]
pub struct HubEndpoints {
    pub hf_endpoint: String,
    pub models_cat_endpoint: String,
}

impl Default for HubEndpoints {
    fn default() -> Self {
        Self {
            hf_endpoint: "https://huggingface.co".to_owned(),
            models_cat_endpoint: "https://www.modelscope.cn".to_owned(),
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct HubClient {
    provider_preference: HubProviderPreference,
    cache_dir: Option<PathBuf>,
    probe_timeout: Duration,
    endpoints: HubEndpoints,
    probe_client: Client,
}

impl Default for HubClient {
    fn default() -> Self {
        let probe_timeout = Duration::from_secs(3);
        let probe_client = Client::builder()
            .timeout(probe_timeout)
            .build()
            .expect("probe client should build");
        Self {
            provider_preference: HubProviderPreference::Auto,
            cache_dir: None,
            probe_timeout,
            endpoints: HubEndpoints::default(),
            probe_client,
        }
    }
}

impl HubClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider_preference(mut self, provider_preference: HubProviderPreference) -> Self {
        self.provider_preference = provider_preference;
        self
    }

    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    pub fn with_probe_timeout(mut self, probe_timeout: Duration) -> Self {
        self.probe_timeout = probe_timeout;
        self.probe_client = Client::builder()
            .timeout(probe_timeout)
            .build()
            .expect("probe client should build");
        self
    }

    pub fn with_hf_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoints.hf_endpoint = endpoint.into();
        self
    }

    pub fn with_models_cat_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoints.models_cat_endpoint = endpoint.into();
        self
    }

    pub async fn list_repo_files(&self, repo_id: &str) -> Result<Vec<String>, HubError> {
        self.run_with_provider_fallback(|provider| async move {
            match provider {
                #[cfg(feature = "provider-hf-hub")]
                HubProvider::HfHub => self.list_repo_files_with_hf_hub(repo_id).await,
                #[cfg(feature = "provider-models-cat")]
                HubProvider::ModelsCat => self.list_repo_files_with_models_cat(repo_id).await,
                #[cfg(feature = "provider-huggingface-hub-rust")]
                HubProvider::HuggingfaceHubRust => {
                    self.list_repo_files_with_huggingface_hub_rust(repo_id).await
                }
                #[allow(unreachable_patterns)]
                other => Err(HubError::new(
                    HubErrorKind::UnsupportedProvider,
                    Some(other),
                    format!("hub provider '{other}' was requested but is not enabled"),
                )),
            }
        })
        .await
    }

    pub async fn download_file(
        &self,
        repo_id: &str,
        filename: &str,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<PathBuf, HubError> {
        self.run_with_provider_fallback(|provider| {
            let progress = progress.clone();
            async move {
            match provider {
                #[cfg(feature = "provider-hf-hub")]
                HubProvider::HfHub => {
                    self.download_file_with_hf_hub(repo_id, filename, progress.clone()).await
                }
                #[cfg(feature = "provider-models-cat")]
                HubProvider::ModelsCat => {
                    self.download_file_with_models_cat(repo_id, filename, progress.clone()).await
                }
                #[cfg(feature = "provider-huggingface-hub-rust")]
                HubProvider::HuggingfaceHubRust => {
                    self.download_file_with_huggingface_hub_rust(repo_id, filename).await
                }
                #[allow(unreachable_patterns)]
                other => Err(HubError::new(
                    HubErrorKind::UnsupportedProvider,
                    Some(other),
                    format!("hub provider '{other}' was requested but is not enabled"),
                )),
            }
        }
        })
        .await
    }

    async fn run_with_provider_fallback<T, F, Fut>(&self, mut operation: F) -> Result<T, HubError>
    where
        F: FnMut(HubProvider) -> Fut,
        Fut: Future<Output = Result<T, HubError>>,
    {
        let candidates = self.enabled_providers()?;
        let is_explicit = matches!(self.provider_preference, HubProviderPreference::Provider(_));
        let mut last_error = None;

        for provider in candidates {
            if !is_explicit {
                match self.probe_provider(provider).await {
                    Ok(()) => {}
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                }
            }

            match operation(provider).await {
                Ok(result) => return Ok(result),
                Err(error)
                    if !is_explicit && matches!(error.kind(), HubErrorKind::NetworkUnavailable) =>
                {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            HubError::new(
                HubErrorKind::NetworkUnavailable,
                None,
                format!(
                    "no enabled hub provider is reachable within {:?}",
                    self.probe_timeout
                ),
            )
        }))
    }

    fn enabled_providers(&self) -> Result<Vec<HubProvider>, HubError> {
        let providers = match self.provider_preference {
            HubProviderPreference::Auto => {
                let mut providers = Vec::new();
                #[cfg(feature = "provider-hf-hub")]
                providers.push(HubProvider::HfHub);
                #[cfg(feature = "provider-models-cat")]
                providers.push(HubProvider::ModelsCat);
                #[cfg(feature = "provider-huggingface-hub-rust")]
                providers.push(HubProvider::HuggingfaceHubRust);
                providers
            }
            HubProviderPreference::Provider(provider) => vec![provider],
        };

        if providers.is_empty() {
            return Err(HubError::new(
                HubErrorKind::UnsupportedProvider,
                None,
                "no hub provider feature is enabled",
            ));
        }

        if let HubProviderPreference::Provider(provider) = self.provider_preference {
            if !providers.contains(&provider) {
                return Err(HubError::new(
                    HubErrorKind::UnsupportedProvider,
                    Some(provider),
                    format!("hub provider '{provider}' is not enabled in this build"),
                ));
            }
        }

        Ok(providers)
    }

    async fn probe_provider(&self, provider: HubProvider) -> Result<(), HubError> {
        let response = self
            .probe_client
            .get(provider.base_url(&self.endpoints))
            .send()
            .await
            .map_err(|error| map_reqwest_error(provider, "provider probe failed", error))?;
        response
            .error_for_status()
            .map_err(|error| map_reqwest_error(provider, "provider probe failed", error))?;
        Ok(())
    }

    #[cfg(feature = "provider-hf-hub")]
    async fn list_repo_files_with_hf_hub(&self, repo_id: &str) -> Result<Vec<String>, HubError> {
        let api = self.hf_hub_api(HubProvider::HfHub)?;
        let repo = api.model(repo_id.to_owned());
        let info = repo.info().await.map_err(|error| map_hf_hub_error(HubProvider::HfHub, "list repo files failed", error))?;
        Ok(info.siblings.into_iter().map(|item| item.rfilename).collect())
    }

    #[cfg(feature = "provider-hf-hub")]
    async fn download_file_with_hf_hub(
        &self,
        repo_id: &str,
        filename: &str,
        progress: Option<Arc<dyn DownloadProgress>>,
    ) -> Result<PathBuf, HubError> {
        let api = self.hf_hub_api(HubProvider::HfHub)?;
        let repo = api.model(repo_id.to_owned());
        match progress {
            Some(progress) => {
                let adapter = HfHubProgressAdapter::new(HubProvider::HfHub, repo_id, filename, progress);
                repo.download_with_progress(filename, adapter)
                    .await
                    .map_err(|error| map_hf_hub_error(HubProvider::HfHub, format!("download failed for {filename}"), error))
            }
            None => repo
                .get(filename)
                .await
                .map_err(|error| map_hf_hub_error(HubProvider::HfHub, format!("download failed for {filename}"), error)),
        }
    }

    #[cfg(feature = "provider-hf-hub")]
    fn hf_hub_api(&self, provider: HubProvider) -> Result<hf_hub::api::tokio::Api, HubError> {
        let mut builder = hf_hub::api::tokio::ApiBuilder::from_env()
            .with_progress(false)
            .with_endpoint(self.endpoints.hf_endpoint.clone());
        if let Some(cache_dir) = self.cache_dir.clone() {
            builder = builder.with_cache_dir(cache_dir);
        }
        builder
            .build()
            .map_err(|error| map_hf_hub_error(provider, "failed to initialize hf-hub client", error))
    }

    #[cfg(feature = "provider-models-cat")]
    async fn list_repo_files_with_models_cat(&self, repo_id: &str) -> Result<Vec<String>, HubError> {
        let repo = self.models_cat_repo(repo_id);
        let client = models_cat::asynchronous::ModelsCat::new_with_endpoint(
            repo,
            self.endpoints.models_cat_endpoint.clone(),
        );
        client
            .list_hub_files()
            .await
            .map_err(|error| map_models_cat_error(HubProvider::ModelsCat, "list repo files failed", error))
    }

    #[cfg(feature = "provider-models-cat")]
    async fn download_file_with_models_cat(
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
                let adapter =
                    ModelsCatProgressAdapter::new(HubProvider::ModelsCat, repo_id, filename, progress);
                client
                    .download_with_progress(filename, adapter)
                    .await
                    .map_err(|error| map_models_cat_error(HubProvider::ModelsCat, format!("download failed for {filename}"), error))?;
            }
            None => {
                client
                    .download(filename)
                    .await
                    .map_err(|error| map_models_cat_error(HubProvider::ModelsCat, format!("download failed for {filename}"), error))?;
            }
        }

        find_models_cat_downloaded_path(&repo, filename).ok_or_else(|| {
            HubError::operation(
                HubProvider::ModelsCat,
                format!("download succeeded for {filename} but no local file was found"),
            )
        })
    }

    #[cfg(feature = "provider-models-cat")]
    fn models_cat_repo(&self, repo_id: &str) -> models_cat::Repo {
        let mut repo = models_cat::Repo::new_model(repo_id);
        if let Some(cache_dir) = self.cache_dir.as_ref() {
            repo.set_cache_dir(cache_dir.join("models-cat"));
        }
        repo
    }

    #[cfg(feature = "provider-huggingface-hub-rust")]
    async fn list_repo_files_with_huggingface_hub_rust(
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
        repo.list_files(&RepoListFilesParams::default())
            .await
            .map_err(|error| {
                map_huggingface_hub_rust_error(
                    HubProvider::HuggingfaceHubRust,
                    "list repo files failed",
                    error,
                )
            })
    }

    #[cfg(feature = "provider-huggingface-hub-rust")]
    async fn download_file_with_huggingface_hub_rust(
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
            &RepoDownloadFileParams::builder()
                .filename(filename)
                .local_dir(local_dir)
                .build(),
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

    #[cfg(feature = "provider-huggingface-hub-rust")]
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

#[cfg(feature = "provider-huggingface-hub-rust")]
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

#[cfg(feature = "provider-models-cat")]
fn find_models_cat_downloaded_path(repo: &models_cat::Repo, filename: &str) -> Option<PathBuf> {
    let base_path = repo.cache_dir().join("snapshots");
    let target = Path::new(filename);

    walkdir::WalkDir::new(&base_path)
        .min_depth(2)
        .max_depth(16)
        .into_iter()
        .filter_map(Result::ok)
        .find_map(|entry| {
            entry
                .file_type()
                .is_file()
                .then_some(entry.path().to_path_buf())
                .filter(|path| path.ends_with(target))
        })
}

fn is_network_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    [
        "connection",
        "dns",
        "network",
        "timed out",
        "timeout",
        "tls",
        "refused",
        "unreachable",
        "reset by peer",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn is_network_io_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrInUse
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
    )
}

fn map_reqwest_error(provider: HubProvider, context: impl Into<String>, error: reqwest::Error) -> HubError {
    let context = context.into();
    let message = format!("{context}: {error}");
    let kind = if is_reqwest_network_error(&error) {
        HubErrorKind::NetworkUnavailable
    } else {
        HubErrorKind::OperationFailed
    };
    HubError::new(kind, Some(provider), message)
}

fn is_reqwest_network_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || (error.is_request() && !error.is_status())
}

fn is_networkish_error_message(message: &str) -> bool {
    is_network_message(message)
}

#[cfg(feature = "provider-hf-hub")]
fn map_hf_hub_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: hf_hub::api::tokio::ApiError,
) -> HubError {
    let context = context.into();
    let kind = match &error {
        hf_hub::api::tokio::ApiError::RequestError(reqwest_error)
            if is_networkish_error_message(&reqwest_error.to_string()) =>
        {
            HubErrorKind::NetworkUnavailable
        }
        hf_hub::api::tokio::ApiError::IoError(io_error) if is_network_io_error(io_error) => {
            HubErrorKind::NetworkUnavailable
        }
        _ => HubErrorKind::OperationFailed,
    };
    HubError::new(kind, Some(provider), format!("{context}: {error}"))
}

#[cfg(feature = "provider-models-cat")]
fn map_models_cat_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: models_cat::OpsError,
) -> HubError {
    let context = context.into();
    let kind = match &error {
        models_cat::OpsError::RequestError(reqwest_error)
            if is_networkish_error_message(&reqwest_error.to_string()) =>
        {
            HubErrorKind::NetworkUnavailable
        }
        models_cat::OpsError::IoError(io_error) if is_network_io_error(io_error) => {
            HubErrorKind::NetworkUnavailable
        }
        _ => HubErrorKind::OperationFailed,
    };
    HubError::new(kind, Some(provider), format!("{context}: {error}"))
}

#[cfg(feature = "provider-huggingface-hub-rust")]
fn map_huggingface_hub_rust_error(
    provider: HubProvider,
    context: impl Into<String>,
    error: huggingface_hub::HFError,
) -> HubError {
    let message = error.to_string();
    let kind = if is_network_message(&message) {
        HubErrorKind::NetworkUnavailable
    } else {
        HubErrorKind::OperationFailed
    };
    HubError::new(kind, Some(provider), format!("{}: {message}", context.into()))
}

#[cfg(feature = "provider-hf-hub")]
#[derive(Clone)]
struct HfHubProgressAdapter {
    observer: Arc<dyn DownloadProgress>,
    state: Arc<Mutex<DownloadProgressUpdate>>,
}

#[cfg(feature = "provider-hf-hub")]
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

#[cfg(feature = "provider-hf-hub")]
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

#[cfg(feature = "provider-models-cat")]
#[derive(Clone)]
struct ModelsCatProgressAdapter {
    observer: Arc<dyn DownloadProgress>,
    state: Arc<Mutex<DownloadProgressUpdate>>,
}

#[cfg(feature = "provider-models-cat")]
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

#[cfg(feature = "provider-models-cat")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_aliases() {
        assert_eq!("hf".parse::<HubProvider>().ok(), Some(HubProvider::HfHub));
        assert_eq!(
            "models_cat".parse::<HubProvider>().ok(),
            Some(HubProvider::ModelsCat)
        );
        assert_eq!(
            "huggingface_hub".parse::<HubProvider>().ok(),
            Some(HubProvider::HuggingfaceHubRust)
        );
    }

    #[test]
    fn auto_provider_preference_normalizes_blank_values() {
        assert_eq!(
            HubProviderPreference::from_optional_str(None).unwrap(),
            HubProviderPreference::Auto
        );
        assert_eq!(
            HubProviderPreference::from_optional_str(Some(" auto ")).unwrap(),
            HubProviderPreference::Auto
        );
    }

    #[test]
    fn explicit_provider_preference_disables_fallback_order_changes() {
        let providers = HubClient::new()
            .with_provider_preference(HubProviderPreference::Provider(HubProvider::HfHub))
            .enabled_providers()
            .expect("providers");
        assert_eq!(providers, vec![HubProvider::HfHub]);
    }

    #[test]
    fn auto_provider_preference_uses_default_enabled_order() {
        let providers = HubClient::new().enabled_providers().expect("providers");
        let mut expected = Vec::new();
        #[cfg(feature = "provider-hf-hub")]
        expected.push(HubProvider::HfHub);
        #[cfg(feature = "provider-models-cat")]
        expected.push(HubProvider::ModelsCat);
        #[cfg(feature = "provider-huggingface-hub-rust")]
        expected.push(HubProvider::HuggingfaceHubRust);
        assert_eq!(providers, expected);
    }
}
