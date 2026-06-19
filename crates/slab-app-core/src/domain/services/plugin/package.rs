use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use reqwest::Url;
use slab_utils::path::ensure_within_root_or_nearest;
use uuid::Uuid;
use zip::ZipArchive;

use crate::error::AppCoreError;

pub(super) async fn load_package_bytes(source: &str) -> Result<Vec<u8>, AppCoreError> {
    if let Ok(url) = Url::parse(source) {
        match url.scheme() {
            "http" | "https" => {
                let response = reqwest::get(source).await.map_err(|error| {
                    AppCoreError::Internal(format!("failed to download plugin package: {error}"))
                })?;
                let status = response.status();
                if !status.is_success() {
                    return Err(AppCoreError::BadRequest(format!(
                        "plugin package download failed with HTTP {status}"
                    )));
                }
                return response.bytes().await.map(|body| body.to_vec()).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to read downloaded plugin package bytes: {error}"
                    ))
                });
            }
            "file" => {
                let path = url.to_file_path().map_err(|_| {
                    AppCoreError::BadRequest(format!(
                        "unsupported file URL for plugin package: {source}"
                    ))
                })?;
                return fs::read(path).map_err(|error| {
                    AppCoreError::Internal(format!(
                        "failed to read plugin package from disk: {error}"
                    ))
                });
            }
            _ => {}
        }
    }

    fs::read(source).map_err(|error| {
        AppCoreError::Internal(format!("failed to read plugin package `{source}`: {error}"))
    })
}

pub(super) async fn run_bun_install(dir: &Path) {
    let result = tokio::process::Command::new("bun")
        .arg("install")
        .arg("--production")
        .current_dir(dir)
        .output()
        .await;
    match result {
        Ok(output) if output.status.success() => {
            tracing::info!("bun install succeeded in {}", dir.display());
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("bun install failed in {}: {stderr}", dir.display());
        }
        Err(error) => {
            tracing::warn!("bun install could not be launched in {}: {error}", dir.display());
        }
    }
}

pub(super) fn create_staging_dir(plugins_dir: &Path) -> Result<PathBuf, AppCoreError> {
    let path = plugins_dir.join(format!(".staging-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create plugin staging directory {}: {error}",
            path.display()
        ))
    })?;
    Ok(path)
}

pub(super) fn extract_plugin_pack_archive(bytes: &[u8], dest: &Path) -> Result<(), AppCoreError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        AppCoreError::BadRequest(format!("failed to open .plugin.slab archive: {error}"))
    })?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            AppCoreError::BadRequest(format!(
                "failed to access .plugin.slab entry #{index}: {error}"
            ))
        })?;
        let Some(path) = file.enclosed_name().map(|value| value.to_path_buf()) else {
            continue;
        };
        let target = dest.join(path);
        if file.is_dir() {
            fs::create_dir_all(&target).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create plugin directory {}: {error}",
                    target.display()
                ))
            })?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppCoreError::Internal(format!(
                    "failed to create plugin parent directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        let mut output = fs::File::create(&target).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create extracted plugin file {}: {error}",
                target.display()
            ))
        })?;
        std::io::copy(&mut file, &mut output).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to write extracted plugin file {}: {error}",
                target.display()
            ))
        })?;
    }

    Ok(())
}

pub(super) fn locate_plugin_root(staging_root: &Path) -> Result<PathBuf, AppCoreError> {
    if staging_root.join("plugin.json").is_file() {
        return Ok(staging_root.to_path_buf());
    }

    let mut manifests = Vec::new();
    collect_manifest_parents(staging_root, &mut manifests)?;
    manifests.sort();
    manifests.dedup();

    match manifests.as_slice() {
        [only] => Ok(only.clone()),
        [] => Err(AppCoreError::BadRequest("plugin pack does not contain plugin.json".to_owned())),
        _ => Err(AppCoreError::BadRequest(
            "plugin pack contains multiple plugin.json files".to_owned(),
        )),
    }
}

fn collect_manifest_parents(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), AppCoreError> {
    for entry in fs::read_dir(root).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to scan extracted plugin directory {}: {error}",
            root.display()
        ))
    })? {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!("failed to read extracted plugin entry: {error}"))
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.join("plugin.json").is_file() {
                output.push(path.clone());
            }
            collect_manifest_parents(&path, output)?;
        }
    }
    Ok(())
}

pub(super) fn move_directory(from: &Path, to: &Path) -> Result<(), AppCoreError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to create plugin destination {}: {error}",
                parent.display()
            ))
        })?;
    }
    fs::rename(from, to).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to move plugin directory from {} to {}: {error}",
            from.display(),
            to.display()
        ))
    })
}

pub(super) fn safe_remove_dir(path: &Path) -> Result<(), AppCoreError> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| {
            AppCoreError::Internal(format!(
                "failed to remove plugin directory {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

pub(super) fn ensure_path_within(path: &Path, root: &Path) -> Result<(), AppCoreError> {
    ensure_within_root_or_nearest(root, path).map(|_| ()).map_err(|error| match error {
        slab_utils::path::EnsureWithinRootError::RootCanonicalize { path, source } => {
            AppCoreError::Internal(format!(
                "failed to resolve plugins root {}: {source}",
                path.display()
            ))
        }
        slab_utils::path::EnsureWithinRootError::PathCanonicalize { path, source } => {
            AppCoreError::Internal(format!(
                "failed to resolve plugin path {}: {source}",
                path.display()
            ))
        }
        slab_utils::path::EnsureWithinRootError::OutsideRoot { root, path } => {
            AppCoreError::BadRequest(format!(
                "plugin path {} escapes plugins root {}",
                path.display(),
                root.display()
            ))
        }
    })
}
