use std::path::PathBuf;
use std::sync::Arc;

use slab_plugin::PluginRegistry;

use super::types::PluginInfo;
use crate::paths::settings_path_from_env_or_default;
pub use slab_plugin::{LoadedPlugin, is_path_within_root, normalize_relative_path};

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
    Ok(plugin_install_dir_from_settings(&settings_path).unwrap_or_else(default_plugins_dir))
}

fn settings_path_for_plugins() -> PathBuf {
    settings_path_from_env_or_default()
}

fn default_plugins_dir() -> PathBuf {
    slab_utils::app_home::plugins_dir()
}

fn plugin_install_dir_from_settings(settings_path: &std::path::Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(settings_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .pointer("/plugin/install_dir")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
