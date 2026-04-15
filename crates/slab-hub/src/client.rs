use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;

use crate::endpoints::HubEndpoints;
use crate::error::{HubError, HubErrorKind, map_reqwest_error};
use crate::progress::DownloadProgress;
use crate::provider::{HubProvider, HubProviderPreference};

#[derive(Debug, Clone)]
pub struct HubClient {
    provider_preference: HubProviderPreference,
    pub(crate) cache_dir: Option<PathBuf>,
    probe_timeout: Duration,
    pub(crate) endpoints: HubEndpoints,
    probe_client: Client,
    last_successful_provider: Arc<Mutex<Option<HubProvider>>>,
}

impl Default for HubClient {
    fn default() -> Self {
        let probe_timeout = Duration::from_secs(3);
        let probe_client =
            Client::builder().timeout(probe_timeout).build().expect("probe client should build");
        Self {
            provider_preference: HubProviderPreference::Auto,
            cache_dir: None,
            probe_timeout,
            endpoints: HubEndpoints::default(),
            probe_client,
            last_successful_provider: Arc::new(Mutex::new(None)),
        }
    }
}

impl HubClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider_preference(mut self, provider_preference: HubProviderPreference) -> Self {
        self.provider_preference = provider_preference;
        self.clear_cached_provider();
        self
    }

    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    pub fn with_probe_timeout(mut self, probe_timeout: Duration) -> Self {
        self.probe_timeout = probe_timeout;
        self.probe_client =
            Client::builder().timeout(probe_timeout).build().expect("probe client should build");
        self.clear_cached_provider();
        self
    }

    pub fn with_hf_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoints.hf_endpoint = endpoint.into();
        self.clear_cached_provider();
        self
    }

    pub fn with_models_cat_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoints.models_cat_endpoint = endpoint.into();
        self.clear_cached_provider();
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
                        self.download_file_with_models_cat(repo_id, filename, progress.clone())
                            .await
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

    pub(crate) async fn run_with_provider_fallback<T, F, Fut>(
        &self,
        mut operation: F,
    ) -> Result<T, HubError>
    where
        F: FnMut(HubProvider) -> Fut,
        Fut: Future<Output = Result<T, HubError>>,
    {
        let candidates = self.enabled_providers()?;
        let is_explicit = matches!(self.provider_preference, HubProviderPreference::Provider(_));
        let cached_provider = (!is_explicit).then(|| self.cached_provider()).flatten();
        let mut last_error = None;

        for provider in candidates {
            let should_probe = !is_explicit && cached_provider != Some(provider);
            if should_probe {
                match self.probe_provider(provider).await {
                    Ok(()) => {}
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                }
            }

            match operation(provider).await {
                Ok(result) => {
                    if !is_explicit {
                        self.set_cached_provider(Some(provider));
                    }
                    return Ok(result);
                }
                Err(error)
                    if !is_explicit && matches!(error.kind(), HubErrorKind::NetworkUnavailable) =>
                {
                    if cached_provider == Some(provider) {
                        self.clear_cached_provider();
                    }
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            HubError::new(
                HubErrorKind::NetworkUnavailable,
                None,
                format!("no enabled hub provider is reachable within {:?}", self.probe_timeout),
            )
        }))
    }

    pub(crate) fn enabled_providers(&self) -> Result<Vec<HubProvider>, HubError> {
        let providers = match self.provider_preference {
            HubProviderPreference::Auto => self.default_enabled_providers(),
            HubProviderPreference::Provider(provider) => {
                if !self.is_provider_enabled(provider) {
                    return Err(HubError::new(
                        HubErrorKind::UnsupportedProvider,
                        Some(provider),
                        format!("hub provider '{provider}' is not enabled in this build"),
                    ));
                }
                vec![provider]
            }
        };

        if providers.is_empty() {
            return Err(HubError::new(
                HubErrorKind::UnsupportedProvider,
                None,
                "no hub provider feature is enabled",
            ));
        }

        if matches!(self.provider_preference, HubProviderPreference::Auto)
            && let Some(cached_provider) = self.cached_provider()
        {
            if let Some(index) = providers.iter().position(|provider| *provider == cached_provider)
            {
                let mut providers = providers;
                let provider = providers.remove(index);
                providers.insert(0, provider);
                return Ok(providers);
            } else {
                self.clear_cached_provider();
            }
        }

        Ok(providers)
    }

    fn default_enabled_providers(&self) -> Vec<HubProvider> {
        let providers: &[HubProvider] = &[
            #[cfg(feature = "provider-hf-hub")]
            HubProvider::HfHub,
            #[cfg(feature = "provider-models-cat")]
            HubProvider::ModelsCat,
            #[cfg(feature = "provider-huggingface-hub-rust")]
            HubProvider::HuggingfaceHubRust,
        ];
        providers.to_vec()
    }

    fn is_provider_enabled(&self, provider: HubProvider) -> bool {
        match provider {
            HubProvider::HfHub => cfg!(feature = "provider-hf-hub"),
            HubProvider::ModelsCat => cfg!(feature = "provider-models-cat"),
            HubProvider::HuggingfaceHubRust => cfg!(feature = "provider-huggingface-hub-rust"),
        }
    }

    pub(crate) fn cached_provider(&self) -> Option<HubProvider> {
        *self.last_successful_provider.lock().expect("cached provider lock")
    }

    pub(crate) fn set_cached_provider(&self, provider: Option<HubProvider>) {
        *self.last_successful_provider.lock().expect("cached provider lock") = provider;
    }

    fn clear_cached_provider(&self) {
        self.set_cached_provider(None);
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
}
