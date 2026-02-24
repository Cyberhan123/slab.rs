use crate::error::FetchError;
use flate2::read::GzDecoder;
use reqwest::Client;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tar::Archive;

pub struct Downloader {
    pub repo: String,
    pub retry_count: usize,
    pub retry_delay_secs: u64,
    pub proxy: Option<String>,
    pub show_progress: bool,
    client: Client,
}

impl Downloader {
    pub fn new(
        repo: &str,
        retry_count: usize,
        retry_delay_secs: u64,
        proxy: Option<String>,
        show_progress: bool,
    ) -> Self {
        let mut builder =
            Client::builder().user_agent(concat!("slab-libfetch/", env!("CARGO_PKG_VERSION")));

        if let Some(ref proxy_url) = proxy {
            match reqwest::Proxy::all(proxy_url) {
                Ok(p) => {
                    builder = builder.proxy(p);
                }
                Err(e) => {
                    eprintln!(
                        "slab-libfetch: ignoring invalid proxy URL {:?}: {}",
                        proxy_url, e
                    );
                }
            }
        }

        let client = match builder.build() {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "slab-libfetch: failed to build HTTP client (falling back to default): {}",
                    e
                );
                Client::new()
            }
        };

        Self {
            repo: repo.to_string(),
            retry_count,
            retry_delay_secs,
            proxy,
            show_progress,
            client,
        }
    }

    /// Fetch the latest release tag from GitHub for the configured repo.
    pub async fn latest_version(&self) -> Result<String, FetchError> {
        let api_url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);

        let mut last_err: FetchError = FetchError::InvalidResponse {
            message: "unable to fetch latest version".to_string(),
        };
        for attempt in 0..self.retry_count {
            match self.get_latest_version_once(&api_url).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    last_err = e;
                    if attempt + 1 < self.retry_count {
                        tokio::time::sleep(Duration::from_secs(self.retry_delay_secs)).await;
                    }
                }
            }
        }
        Err(last_err)
    }

    async fn get_latest_version_once(&self, url: &str) -> Result<String, FetchError> {
        let resp = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?
            .error_for_status()?;

        let json: serde_json::Value = resp.json().await?;
        json["tag_name"]
            .as_str()
            .ok_or_else(|| FetchError::InvalidResponse {
                message: "tag_name not found in GitHub API response".to_string(),
            })
            .map(|s| s.to_string())
    }

    /// Build the download URL for a release asset.
    pub fn asset_url(&self, asset_name: &str, version: &str) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            self.repo, version, asset_name
        )
    }

    /// Download and extract a release asset into `dest`.
    pub async fn download_asset(
        &self,
        asset_name: &str,
        version: &str,
        dest: &Path,
    ) -> Result<(), FetchError> {
        let url = self.asset_url(asset_name, version);

        if self.show_progress {
            println!("🚀 正在从 {} 下载...", url);
        }

        let bytes = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        std::fs::create_dir_all(dest)?;

        if asset_name.ends_with(".zip") {
            extract_zip(&bytes, dest)?;
        } else if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
            extract_tar_gz_strip_top(&bytes, dest)?;
        } else {
            std::fs::write(dest.join(asset_name), &bytes)?;
        }

        if self.show_progress {
            println!("✨ {} 下载完成。", asset_name);
        }

        Ok(())
    }

    /// Download the source tarball for `version` and extract header files into `dest`.
    ///
    /// Prefers the `include/` sub-directory; if none is found, falls back to
    /// extracting every `.h`, `.hpp`, and `.hxx` file in the archive.
    pub async fn download_source_headers(
        &self,
        version: &str,
        dest: &Path,
    ) -> Result<(), FetchError> {
        let tarball_url = format!(
            "https://github.com/{}/archive/refs/tags/{}.tar.gz",
            self.repo, version
        );

        if self.show_progress {
            println!("🚀 正在从 {} 下载...", tarball_url);
        }

        let bytes = self
            .client
            .get(&tarball_url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        std::fs::create_dir_all(dest)?;
        extract_source_headers(&bytes, dest, self.show_progress)?;
        Ok(())
    }
}

/// Extract a ZIP archive into `dest`, stripping the top-level directory.
///
/// Archives are expected to contain a single top-level directory (e.g.
/// `repo-v1.0.0/`). Files at the root of the archive (with no directory
/// component) are silently skipped. Only deflate-compressed entries are
/// supported; other compression methods will return an error.
pub(crate) fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), FetchError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_path = match file.enclosed_name() {
            Some(p) => p,
            None => continue,
        };

        let components: Vec<_> = file_path.components().collect();
        if components.len() <= 1 {
            continue;
        }
        let rel_path: std::path::PathBuf = components.iter().skip(1).collect();
        let dest_path = dest.join(rel_path);

        if file.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&dest_path)?;
            std::io::copy(&mut file, &mut out)?;
        }
    }

    Ok(())
}

/// Extract a `.tar.gz` archive into `dest`, stripping the top-level directory.
///
/// Archives are expected to contain a single top-level directory (e.g.
/// `repo-v1.0.0/`). Entries at the archive root (with no directory component)
/// are silently skipped.
pub(crate) fn extract_tar_gz_strip_top(bytes: &[u8], dest: &Path) -> Result<(), FetchError> {
    let tar_gz = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(tar_gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let full_path = entry.path()?.to_path_buf();
        let components: Vec<_> = full_path.components().collect();
        if components.len() <= 1 {
            continue;
        }
        let rel_path: std::path::PathBuf = components.iter().skip(1).collect();
        let dest_path = dest.join(rel_path);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry.unpack(dest_path)?;
    }

    Ok(())
}

/// Extract header files from a source `.tar.gz` into `dest`.
///
/// First pass: extracts everything under an `include/` directory (preserving
/// the `include/` prefix in the destination).  If no `include/` directory is
/// found, a second pass extracts all `.h`, `.hpp`, and `.hxx` files (stripping
/// the archive root directory).
pub(crate) fn extract_source_headers(
    bytes: &[u8],
    dest: &Path,
    show_progress: bool,
) -> Result<(), FetchError> {
    // First pass – prefer the `include/` directory.
    let tar_gz = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(tar_gz);
    let mut has_include_dir = false;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let full_path = entry.path()?.to_path_buf();

        if let Some(rel_path) = extract_include_part(&full_path) {
            has_include_dir = true;
            let dest_path = dest.join(rel_path);
            if let Some(p) = dest_path.parent() {
                std::fs::create_dir_all(p)?;
            }
            entry.unpack(dest_path)?;
        }
    }

    if has_include_dir {
        return Ok(());
    }

    // Second pass – no `include/` dir found; extract all header files.
    if show_progress {
        println!("⚠️  未找到 include 目录，提取所有头文件...");
    }
    let tar_gz = GzDecoder::new(Cursor::new(bytes));
    let mut archive = Archive::new(tar_gz);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let full_path = entry.path()?.to_path_buf();

        if let Some(rel_path) = filter_header_files(&full_path) {
            let dest_path = dest.join(rel_path);
            if let Some(p) = dest_path.parent() {
                std::fs::create_dir_all(p)?;
            }
            entry.unpack(dest_path)?;
        }
    }

    Ok(())
}

/// Return the path starting from the `include/` component, or `None` if there
/// is no `include/` component in `path`.
fn extract_include_part(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();
    comps
        .iter()
        .position(|c| c.as_os_str() == "include")
        .map(|index| comps.iter().skip(index).collect())
}

/// Return the path relative to the archive root directory for `.h`, `.hpp`, and
/// `.hxx` files, skipping the top-level directory.  Returns `None` for
/// directories and non-header files.
fn filter_header_files(path: &Path) -> Option<PathBuf> {
    let comps: Vec<_> = path.components().collect();
    if comps.len() <= 1 {
        return None;
    }
    let name = path.file_name()?.to_string_lossy();
    if name.ends_with(".h") || name.ends_with(".hpp") || name.ends_with(".hxx") {
        Some(comps.iter().skip(1).collect())
    } else {
        None
    }
}
