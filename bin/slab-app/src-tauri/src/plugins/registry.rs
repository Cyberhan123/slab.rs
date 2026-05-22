use std::path::{Path, PathBuf};
use std::sync::Arc;

use dirs_next::config_dir;
use slab_plugin::PluginRegistry;

use super::types::PluginInfo;
pub use slab_plugin::{LoadedPlugin, is_path_within_root, normalize_relative_path};

pub const DEFAULT_PLUGINS_DIR: &str = "plugins";

pub struct PluginRegistryState {
    registry: Arc<PluginRegistry>,
}

impl PluginRegistryState {
    pub fn new(root_dir: PathBuf) -> Result<Self, String> {
        let registry = PluginRegistry::new(root_dir)?;
        Ok(Self { registry: Arc::new(registry) })
    }

    pub fn refresh(&self) -> Result<(), String> {
        self.registry.refresh()
    }

    pub fn get_plugin(&self, plugin_id: &str) -> Result<LoadedPlugin, String> {
        self.registry.get_plugin(plugin_id)
    }

    pub fn list(&self) -> Result<Vec<PluginInfo>, String> {
        self.registry.list()
    }
}

pub fn resolve_plugins_root<R: tauri::Runtime>(_app: &tauri::App<R>) -> Result<PathBuf, String> {
    let settings_path = settings_path_for_plugins();
    Ok(resolve_plugins_root_with(&settings_path, plugin_install_dir_from_settings(&settings_path)))
}

pub fn resolve_plugins_root_for_settings_path(settings_path: &Path) -> PathBuf {
    resolve_plugins_root_with(settings_path, plugin_install_dir_from_settings(settings_path))
}

fn resolve_plugins_root_with(
    settings_path: &Path,
    settings_install_dir: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = settings_install_dir {
        return path;
    }

    default_plugins_dir_for_settings_path(settings_path)
}

fn settings_path_for_plugins() -> PathBuf {
    std::env::var("SLAB_SETTINGS_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(default_settings_path)
}

fn default_settings_path() -> PathBuf {
    config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Slab").join("settings.json")
}

fn default_plugins_dir_for_settings_path(settings_path: &Path) -> PathBuf {
    settings_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_PLUGINS_DIR)
}

fn plugin_install_dir_from_settings(settings_path: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(settings_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .pointer("/plugin/install_dir")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
