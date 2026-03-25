use crate::downloader::Downloader;
use crate::error::FetchError;
use crate::install::{Install, VersionInfo};
use crate::manifest::{Manifest, ResolvedArtifact};
use crate::platform::Platform;
use crate::variant::Variant;
use crate::verify::verify_sha256;
use std::env;
use std::path::{Path, PathBuf};

/// Top-level builder for the libfetch API.
///
/// # Example
/// ```rust,no_run
/// # tokio_test::block_on(async {
/// use slab_libfetch::Api;
/// Api::new()
///     .set_install_dir("./llamalib")
///     .repo("ggml-org/llama.cpp")
///     .latest()
///     .install(|v| format!("llama-{v}-bin-win-cpu-x64.zip"))
///     .await
///     .unwrap();
/// # })
/// ```
pub struct Api {
    pub(crate) install_dir: PathBuf,
    pub(crate) retry_count: usize,
    pub(crate) retry_delay_secs: u64,
    pub(crate) proxy: Option<String>,
    pub(crate) show_progress: bool,
}

/// Builder stage after `.repo()` has been called.
pub struct RepoApi {
    api: Api,
    repo: String,
}

/// Builder stage after `.latest()` or `.version()` has been called.
pub struct VersionApi {
    api: Api,
    repo: String,
    version: String,
    is_latest: bool,
    /// Pre-resolved artifact from a manifest (optional).
    resolved_artifact: Option<ResolvedArtifact>,
}

impl Default for Api {
    fn default() -> Self {
        Self::new()
    }
}

impl Api {
    /// Create a new `Api` instance.
    ///
    /// Proxy is automatically read from `HTTP_PROXY` / `HTTPS_PROXY` environment variables.
    pub fn new() -> Self {
        let proxy = env::var("HTTP_PROXY").ok().or_else(|| env::var("HTTPS_PROXY").ok());

        Self {
            install_dir: PathBuf::from("."),
            retry_count: 3,
            retry_delay_secs: 3,
            proxy,
            show_progress: true,
        }
    }

    /// Set the directory where assets are installed (default: `"."`).
    pub fn set_install_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.install_dir = dir.as_ref().components().collect();
        self
    }

    /// Set the number of retries for GitHub API calls (default: `3`).
    pub fn set_retry_count(mut self, count: usize) -> Self {
        self.retry_count = count;
        self
    }

    /// Set the delay in seconds between retries (default: `3`).
    pub fn set_retry_delay_secs(mut self, secs: u64) -> Self {
        self.retry_delay_secs = secs;
        self
    }

    /// Override the HTTP/HTTPS proxy URL.
    pub fn set_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Disable progress output.
    pub fn no_progress(mut self) -> Self {
        self.show_progress = false;
        self
    }

    /// Specify the GitHub repository (`"owner/repo"`).
    pub fn repo(self, repo: impl Into<String>) -> RepoApi {
        RepoApi { api: self, repo: repo.into() }
    }

    /// Create a `VersionApi` pre-configured from a manifest entry.
    ///
    /// Looks up `artifact_name` in `manifest`, auto-detects the current
    /// platform and best variant, and constructs a `VersionApi` ready for
    /// downloading.
    pub fn from_manifest(
        self,
        manifest: &Manifest,
        artifact_name: &str,
    ) -> Result<VersionApi, FetchError> {
        let platform = Platform::current().ok_or_else(|| {
            FetchError::ManifestError("unsupported OS or architecture".to_string())
        })?;
        let variant = Variant::detect_best(&platform.os);
        self.from_manifest_with_platform(manifest, artifact_name, &platform, &variant)
    }

    /// Create a `VersionApi` pre-configured from a manifest entry with an
    /// explicit `platform` and `variant`.
    pub fn from_manifest_with_platform(
        self,
        manifest: &Manifest,
        artifact_name: &str,
        platform: &Platform,
        variant: &Variant,
    ) -> Result<VersionApi, FetchError> {
        let spec = manifest.artifact(artifact_name)?;
        let resolved = spec.resolve(platform, variant)?;
        Ok(VersionApi {
            api: self,
            repo: resolved.repo.clone(),
            version: resolved.version.clone(),
            is_latest: false,
            resolved_artifact: Some(resolved),
        })
    }
}

impl RepoApi {
    /// Target the latest release.
    pub fn latest(self) -> VersionApi {
        VersionApi {
            api: self.api,
            repo: self.repo,
            version: String::new(),
            is_latest: true,
            resolved_artifact: None,
        }
    }

    /// Target a specific release tag (e.g. `"v3.5.1"`).
    pub fn version(self, version: impl Into<String>) -> VersionApi {
        VersionApi {
            api: self.api,
            repo: self.repo,
            version: version.into(),
            is_latest: false,
            resolved_artifact: None,
        }
    }

    /// Return the installed version information from `version.json`.
    pub fn get_installed_version(&self) -> Result<VersionInfo, FetchError> {
        Install::new(&self.repo, &self.api.install_dir).get_installed_version()
    }
}

impl VersionApi {
    /// Download and extract the release asset produced by `asset_func(version)`.
    ///
    /// `asset_func` receives the resolved version tag and must return the asset file name.
    pub async fn install<F>(self, asset_func: F) -> Result<PathBuf, FetchError>
    where
        F: Fn(&str) -> String,
    {
        let downloader = Downloader::new(
            &self.repo,
            self.api.retry_count,
            self.api.retry_delay_secs,
            self.api.proxy.clone(),
            self.api.show_progress,
        );

        // Resolve the version early so asset_func can use it.
        let version =
            if self.is_latest { downloader.latest_version().await? } else { self.version.clone() };

        let asset_name = asset_func(&version);

        let install = Install::new(&self.repo, &self.api.install_dir);
        install.install_asset(&downloader, &asset_name, &version, self.is_latest).await
    }

    /// Download and extract the artifact described by a previously resolved
    /// manifest entry, with SHA256 checksum verification.
    ///
    /// This method is intended for use after creating a `VersionApi` via
    /// [`Api::from_manifest`] or [`Api::from_manifest_with_platform`].  It
    /// uses the pre-resolved asset name from the manifest and verifies the
    /// downloaded bytes against the checksum (if one is present in the
    /// manifest).  When no checksum is declared a tracing warning is emitted.
    ///
    /// `platform` and `variant` are used only for progress output.
    pub async fn install_with_platform(
        self,
        platform: &Platform,
        variant: &Variant,
    ) -> Result<PathBuf, FetchError> {
        let resolved = self.resolved_artifact.as_ref().ok_or_else(|| {
            FetchError::ManifestError(
                "install_with_platform requires a VersionApi created via Api::from_manifest"
                    .to_string(),
            )
        })?;

        let downloader = Downloader::new(
            &self.repo,
            self.api.retry_count,
            self.api.retry_delay_secs,
            self.api.proxy.clone(),
            self.api.show_progress,
        );

        if self.api.show_progress {
            println!(
                "🚀 Installing {} for {}-{} ({} variant)…",
                resolved.asset_name, platform.os, platform.arch, variant
            );
        }

        let install = Install::new(&self.repo, &self.api.install_dir);

        // Skip if already at the right version.
        if install.already_installed() {
            if let Ok(info) = install.get_installed_version() {
                if info.tag_name == resolved.version && info.repo == self.repo {
                    if self.api.show_progress {
                        println!("✅ Version {} already installed, skipping.", resolved.version);
                    }
                    return Ok(self.api.install_dir.clone());
                }
            }
            // Different version — reinstall.
            if self.api.install_dir.exists() {
                std::fs::remove_dir_all(&self.api.install_dir)?;
            }
        }

        // Download the raw bytes so we can verify before extracting.
        let bytes = downloader.download_asset_bytes(&resolved.asset_name, &resolved.version).await?;

        // Checksum verification.
        match &resolved.checksum {
            Some(expected) => {
                verify_sha256(&bytes, expected)?;
                if self.api.show_progress {
                    println!("✅ Checksum verified.");
                }
            }
            None => {
                tracing::warn!(
                    asset = %resolved.asset_name,
                    "no checksum declared in manifest; skipping integrity verification"
                );
            }
        }

        std::fs::create_dir_all(&self.api.install_dir)?;
        if resolved.asset_name.ends_with(".zip") {
            crate::downloader::extract_zip(&bytes, &self.api.install_dir)?;
        } else if resolved.asset_name.ends_with(".tar.gz")
            || resolved.asset_name.ends_with(".tgz")
        {
            crate::downloader::extract_tar_gz_strip_top(&bytes, &self.api.install_dir)?;
        } else {
            std::fs::write(
                self.api.install_dir.join(&resolved.asset_name),
                &bytes,
            )?;
        }

        install.create_version_file(&resolved.version)?;

        if self.api.show_progress {
            println!("✨ {} installed successfully.", resolved.asset_name);
        }

        Ok(self.api.install_dir.clone())
    }

    /// Download header files from the source tarball and extract them to
    /// `target_path`.
    ///
    /// Prefers the `include/` sub-directory inside the archive; if none is
    /// found, falls back to extracting every `.h`, `.hpp`, and `.hxx` file.
    /// Skips the download entirely when `version.json` already records the
    /// same version.
    pub async fn fetch_header(self, target_path: &Path) -> Result<(), FetchError> {
        let downloader = Downloader::new(
            &self.repo,
            self.api.retry_count,
            self.api.retry_delay_secs,
            self.api.proxy.clone(),
            self.api.show_progress,
        );

        let version = if self.is_latest {
            if self.api.show_progress {
                println!("🔍 未指定版本，正在获取 {} 的最新版本...", self.repo);
            }
            downloader.latest_version().await?
        } else {
            self.version.clone()
        };

        // Skip if already at this version.
        let install = Install::new_with_path(&self.repo, target_path);
        if install.already_installed() {
            if let Ok(info) = install.get_installed_version() {
                if info.tag_name == version {
                    if self.api.show_progress {
                        println!("✅ 版本 {} 的头文件已存在，跳过下载。", version);
                    }
                    return Ok(());
                }
            }
            // Different version – clean up before re-downloading.
            std::fs::remove_dir_all(target_path)?;
        }

        downloader.download_source_headers(&version, target_path).await?;

        install.create_version_file(&version)?;

        if self.api.show_progress {
            println!("✨ 版本 {} 的头文件已准备就绪。", version);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_defaults() {
        let api = Api::new();
        assert_eq!(api.install_dir, PathBuf::from("."));
        assert_eq!(api.retry_count, 3);
        assert_eq!(api.retry_delay_secs, 3);
        assert!(api.show_progress);
    }

    #[test]
    fn test_api_builder_methods() {
        let api = Api::new()
            .set_install_dir(PathBuf::from("."))
            .set_retry_count(5)
            .set_retry_delay_secs(10)
            .set_proxy("http://proxy:8080")
            .no_progress();

        assert_eq!(api.install_dir, PathBuf::from("."));
        assert_eq!(api.retry_count, 5);
        assert_eq!(api.retry_delay_secs, 10);
        assert_eq!(api.proxy, Some("http://proxy:8080".to_string()));
        assert!(!api.show_progress);
    }

    #[test]
    fn test_repo_returns_repo_api() {
        let repo_api = Api::new().repo("owner/repo");
        assert_eq!(repo_api.repo, "owner/repo");
    }

    #[test]
    fn test_latest_sets_is_latest() {
        let ver = Api::new().repo("owner/repo").latest();
        assert!(ver.is_latest);
        assert!(ver.version.is_empty());
    }

    #[test]
    fn test_version_sets_tag() {
        let ver = Api::new().repo("owner/repo").version("v3.5.1");
        assert!(!ver.is_latest);
        assert_eq!(ver.version, "v3.5.1");
    }
}
