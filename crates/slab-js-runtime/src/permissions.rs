use slab_types::{PluginNetworkMode, PluginPermissionsManifest};

#[derive(Debug, Clone)]
pub struct JsPluginPermissions {
    pub network_mode: PluginNetworkMode,
    pub allow_hosts: Vec<String>,
    pub file_read: Vec<String>,
    pub file_write: Vec<String>,
}

impl Default for JsPluginPermissions {
    fn default() -> Self {
        Self {
            network_mode: PluginNetworkMode::Blocked,
            allow_hosts: Vec::new(),
            file_read: Vec::new(),
            file_write: Vec::new(),
        }
    }
}

impl From<&PluginPermissionsManifest> for JsPluginPermissions {
    fn from(value: &PluginPermissionsManifest) -> Self {
        Self {
            network_mode: value.network.mode.clone(),
            allow_hosts: value.network.allow_hosts.clone(),
            file_read: value.files.read.clone(),
            file_write: value.files.write.clone(),
        }
    }
}
