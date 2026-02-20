use std::env;
use std::path::Path;

use crate::downloader::Downloader;
use crate::install::{Install, VersionInfo};

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
    pub(crate) install_dir: String,
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
        let proxy = env::var("HTTP_PROXY")
            .ok()
            .or_else(|| env::var("HTTPS_PROXY").ok());

        Self {
            install_dir: ".".to_string(),
            retry_count: 3,
            retry_delay_secs: 3,
            proxy,
            show_progress: true,
        }
    }

    /// Set the directory where assets are installed (default: `"."`).
    pub fn set_install_dir(mut self, dir: impl Into<String>) -> Self {
        self.install_dir = dir.into();
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
        RepoApi {
            api: self,
            repo: repo.into(),
        }
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
        }
    }

    /// Target a specific release tag (e.g. `"v3.5.1"`).
    pub fn version(self, version: impl Into<String>) -> VersionApi {
        VersionApi {
            api: self.api,
            repo: self.repo,
            version: version.into(),
            is_latest: false,
        }
    }

    /// Return the installed version information from `version.json`.
    pub fn get_installed_version(&self) -> anyhow::Result<VersionInfo> {
        Install::new(&self.repo, &self.api.install_dir).get_installed_version()
    }
}

impl VersionApi {
    /// Download and extract the release asset produced by `asset_func(version)`.
    ///
    /// `asset_func` receives the resolved version tag and must return the asset file name.
    pub async fn install<F>(self, asset_func: F) -> anyhow::Result<()>
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
        let version = if self.is_latest {
            downloader.latest_version().await?
        } else {
            self.version.clone()
        };

        let asset_name = asset_func(&version);

        let install = Install::new(&self.repo, &self.api.install_dir);
        install
            .install_asset(&downloader, &asset_name, &version, self.is_latest)
            .await
    }

    /// Download header files from the source tarball and extract them to
    /// `target_path`.
    ///
    /// Prefers the `include/` sub-directory inside the archive; if none is
    /// found, falls back to extracting every `.h`, `.hpp`, and `.hxx` file.
    /// Skips the download entirely when `version.json` already records the
    /// same version.
    pub async fn fetch_header(self, target_path: &Path) -> anyhow::Result<()> {
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

        downloader
            .download_source_headers(&version, target_path)
            .await?;

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
        assert_eq!(api.install_dir, ".");
        assert_eq!(api.retry_count, 3);
        assert_eq!(api.retry_delay_secs, 3);
        assert!(api.show_progress);
    }

    #[test]
    fn test_api_builder_methods() {
        let api = Api::new()
            .set_install_dir("./mydir")
            .set_retry_count(5)
            .set_retry_delay_secs(10)
            .set_proxy("http://proxy:8080")
            .no_progress();

        assert_eq!(api.install_dir, "./mydir");
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
