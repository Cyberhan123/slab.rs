use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::downloader::Downloader;

const VERSION_FILE: &str = "version.json";

/// Version information stored in `version.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub tag_name: String,
    pub repo: String,
}

/// Manages installation state and version tracking for a single repository.
pub struct Install {
    repo: String,
    install_path: PathBuf,
}

impl Install {
    pub fn new<P: AsRef<Path>>(repo: &str, install_path: P) -> Self {
        let path = install_path.as_ref().components().collect();
        Self {
            repo: repo.to_string(),
            install_path: path,
        }
    }

    pub fn new_with_path<P: AsRef<Path>>(repo: &str, install_path: P) -> Self {
       let path = install_path.as_ref().components().collect();
        Self {
            repo: repo.to_string(),
            install_path: path,
        }
    }

    fn version_file(&self) -> PathBuf {
        self.install_path.join(VERSION_FILE)
    }

    pub fn already_installed(&self) -> bool {
        self.version_file().exists()
    }

    /// Read the stored `version.json`.
    pub fn get_installed_version(&self) -> Result<VersionInfo> {
        let data = fs::read_to_string(self.version_file())
            .context("Failed to read version.json")?;
        serde_json::from_str(&data).context("Failed to parse version.json")
    }

    /// Write a `version.json` with the given tag and repo.
    pub fn create_version_file(&self, version: &str) -> Result<()> {
        fs::create_dir_all(&self.install_path)?;
        let info = VersionInfo {
            tag_name: version.to_string(),
            repo: self.repo.clone(),
        };
        let data = serde_json::to_string(&info)?;
        fs::write(self.version_file(), data)?;
        Ok(())
    }

    /// Install `asset_name` at `version`.
    ///
    /// - If `allow_upgrade` is true (i.e. `.latest()` was used): check if already at
    ///   the latest and skip; otherwise clean and re-install.
    /// - If `allow_upgrade` is false (i.e. a pinned `.version()` was used): skip if the
    ///   same version is already installed; otherwise clean and re-install.
    pub async fn install_asset(
        &self,
        downloader: &Downloader,
        asset_name: &str,
        version: &str,
        allow_upgrade: bool,
    ) -> Result<PathBuf> {
        if self.already_installed() {
            let installed = self.get_installed_version()?;

            if installed.repo != self.repo {
                anyhow::bail!(
                    "installed version is for a different repository: {}",
                    installed.repo
                );
            }

            if allow_upgrade {
                // Resolve "latest" and skip if already current.
                let latest = downloader.latest_version().await?;
                if latest == installed.tag_name {
                    println!(
                        "✅ 版本 {} 已是最新，跳过下载。",
                        installed.tag_name
                    );
                    return Ok(self.install_path.clone());
                }
                // Upgrade
                self.remove_install_dir()?;
                downloader
                    .download_asset(asset_name, &latest, &self.install_path)
                    .await?;
                self.create_version_file(&latest)?;
            } else {
                // Pinned version – skip if already installed at that version.
                if installed.tag_name == version {
                    println!(
                        "✅ 版本 {} 的资产已存在，跳过下载。",
                        version
                    );
                    return Ok(self.install_path.clone());
                }
                // Different pinned version – reinstall.
                self.remove_install_dir()?;
                downloader
                    .download_asset(asset_name, version, &self.install_path)
                    .await?;
                self.create_version_file(version)?;
            }

            return Ok(self.install_path.clone());
        }

        // First install
        downloader
            .download_asset(asset_name, version, &self.install_path)
            .await?;

        self.create_version_file(version)?;
        Ok(self.install_path.clone())
    }

    fn remove_install_dir(&self) -> Result<()> {
        if self.install_path.exists() {
            fs::remove_dir_all(&self.install_path)
                .context("Failed to remove existing install directory")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_and_read_version_file() {
        let dir = tempdir();
        let install = Install::new("owner/repo", dir.as_str());
        install.create_version_file("v1.0.0").unwrap();

        let info = install.get_installed_version().unwrap();
        assert_eq!(info.tag_name, "v1.0.0");
        assert_eq!(info.repo, "owner/repo");
        assert!(install.already_installed());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_already_installed_false_when_no_dir() {
        let dir = std::env::temp_dir().join("slab_libfetch_nonexistent_test_dir");
        let install = Install::new("owner/repo", dir.to_str().unwrap());
        assert!(!install.already_installed());
    }

    fn tempdir() -> String {
        let path = std::env::temp_dir().join(format!(
            "slab_libfetch_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path.to_str().unwrap().to_string()
    }
}
