use std::fs;
use std::path::{Path, PathBuf};

use slab_types::plugin::PluginManifest;
use slab_utils::hash::sha256_hex_bytes;

use crate::error::AppCoreError;

use super::SOURCE_KIND_DEV;
use super::validation::validate_plugin_manifest;

const IGNORED_PLUGIN_ROOT_NAMES: &[&str] = &["dist", ".git", "node_modules"];
const BUILTIN_LANGUAGE_SERVER_PLUGIN_IDS: &[&str] =
    &["native-language-servers", "web-language-servers"];

#[derive(Debug, Clone)]
pub(super) struct ScannedPlugin {
    pub(super) id: String,
    pub(super) root_dir: PathBuf,
    pub(super) source_kind: String,
    pub(super) valid: bool,
    pub(super) error: Option<String>,
    pub(super) manifest: Option<PluginManifest>,
    pub(super) manifest_hash: Option<String>,
}

pub(super) fn scan_plugins(root_dir: &Path) -> Result<Vec<ScannedPlugin>, AppCoreError> {
    fs::create_dir_all(root_dir).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to create plugins directory {}: {error}",
            root_dir.display()
        ))
    })?;

    let mut rows = Vec::new();
    let entries = fs::read_dir(root_dir).map_err(|error| {
        AppCoreError::Internal(format!(
            "failed to scan plugins directory {}: {error}",
            root_dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            AppCoreError::Internal(format!("failed to read plugins directory entry: {error}"))
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().to_string();
        if is_builtin_language_server_plugin_id(&folder_name) {
            continue;
        }
        if IGNORED_PLUGIN_ROOT_NAMES.iter().any(|ignored| entry.file_name() == *ignored) {
            continue;
        }
        if !path.join("plugin.json").is_file() {
            continue;
        }
        rows.push(scan_plugin_dir(&path, SOURCE_KIND_DEV)?);
    }
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(rows)
}

pub(super) fn scan_plugin_dir(
    root_dir: &Path,
    default_source_kind: &str,
) -> Result<ScannedPlugin, AppCoreError> {
    let fallback_id =
        root_dir.file_name().and_then(|name| name.to_str()).unwrap_or("unknown-plugin").to_owned();
    let manifest_path = root_dir.join("plugin.json");
    let manifest_bytes = match fs::read(&manifest_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Ok(ScannedPlugin {
                id: fallback_id,
                root_dir: root_dir.to_path_buf(),
                source_kind: default_source_kind.to_owned(),
                valid: false,
                error: Some(format!("failed to read {}: {error}", manifest_path.display())),
                manifest: None,
                manifest_hash: None,
            });
        }
    };
    let manifest_hash = sha256_hex_bytes(&manifest_bytes);
    let manifest: PluginManifest = match serde_json::from_slice(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            return Ok(ScannedPlugin {
                id: fallback_id,
                root_dir: root_dir.to_path_buf(),
                source_kind: default_source_kind.to_owned(),
                valid: false,
                error: Some(format!("failed to parse plugin.json: {error}")),
                manifest: None,
                manifest_hash: Some(manifest_hash),
            });
        }
    };
    let plugin_id = manifest.id.clone();
    if let Err(error) = validate_plugin_manifest(root_dir, &manifest, default_source_kind) {
        return Ok(ScannedPlugin {
            id: plugin_id,
            root_dir: root_dir.to_path_buf(),
            source_kind: default_source_kind.to_owned(),
            valid: false,
            error: Some(error),
            manifest: Some(manifest),
            manifest_hash: Some(manifest_hash),
        });
    }
    let scanned = ScannedPlugin {
        id: plugin_id,
        root_dir: root_dir.to_path_buf(),
        source_kind: default_source_kind.to_owned(),
        valid: true,
        error: None,
        manifest: Some(manifest),
        manifest_hash: Some(manifest_hash),
    };
    Ok(scanned)
}

pub(super) fn is_builtin_language_server_plugin_id(plugin_id: &str) -> bool {
    BUILTIN_LANGUAGE_SERVER_PLUGIN_IDS.contains(&plugin_id)
}
