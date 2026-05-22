use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use base64::Engine;
use extism::{Manifest as ExtismManifest, Plugin, PluginBuilder, Wasm};

use crate::error::PluginError;
use crate::runtime::PluginBackend;
use crate::types::{LoadedPlugin, PluginCallRequest, PluginCallResponse, PluginNetworkMode};

struct RuntimeInstance {
    plugin: Plugin,
}

pub struct WasmPluginBackend {
    instances: Mutex<HashMap<String, RuntimeInstance>>,
}

impl WasmPluginBackend {
    pub fn new() -> Self {
        Self { instances: Mutex::new(HashMap::new()) }
    }
}

impl Default for WasmPluginBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginBackend for WasmPluginBackend {
    fn accepts(&self, plugin: &LoadedPlugin) -> bool {
        plugin.wasm_entry_path.is_some()
    }

    async fn call(
        &self,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError> {
        let mut guard = self
            .instances
            .lock()
            .map_err(|_| PluginError::Runtime("failed to lock wasm runtime manager".to_string()))?;

        if !guard.contains_key(&plugin.manifest.id) {
            let runtime = RuntimeInstance { plugin: build_extism_plugin(plugin)? };
            guard.insert(plugin.manifest.id.clone(), runtime);
        }

        let runtime = guard.get_mut(&plugin.manifest.id).ok_or_else(|| {
            PluginError::Runtime("failed to acquire initialized wasm runtime".to_string())
        })?;

        let output = runtime
            .plugin
            .call::<_, Vec<u8>>(request.function.as_str(), request.input.as_bytes())
            .map_err(|error| {
                PluginError::Runtime(format!(
                    "failed to call function `{}` on plugin `{}`: {error}",
                    request.function, request.plugin_id
                ))
            })?;

        Ok(PluginCallResponse {
            output_text: String::from_utf8_lossy(&output).to_string(),
            output_base64: base64::engine::general_purpose::STANDARD.encode(output),
        })
    }
}

fn build_extism_plugin(plugin: &LoadedPlugin) -> Result<Plugin, PluginError> {
    let wasm_entry_path = plugin.wasm_entry_path.as_ref().ok_or_else(|| {
        PluginError::Runtime(format!("plugin `{}` has no wasm runtime", plugin.manifest.id))
    })?;

    let mut manifest = ExtismManifest::new([Wasm::file(wasm_entry_path.clone())]);
    manifest = manifest
        .with_allowed_path(plugin.root_dir.to_string_lossy().to_string(), plugin.root_dir.clone());

    if plugin.manifest.permissions.network.mode == PluginNetworkMode::Allowlist {
        if plugin.manifest.permissions.network.allow_hosts.is_empty() {
            return Err(PluginError::Runtime(format!(
                "plugin `{}` uses allowlist network mode but no allowHosts are configured",
                plugin.manifest.id
            )));
        }
        manifest = manifest.with_allowed_hosts(
            plugin.manifest.permissions.network.allow_hosts.clone().into_iter(),
        );
    }

    PluginBuilder::new(manifest).with_wasi(true).build().map_err(|error| {
        PluginError::Runtime(format!(
            "failed to initialize extism plugin `{}`: {error}",
            plugin.manifest.id
        ))
    })
}
