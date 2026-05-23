use async_trait::async_trait;

use crate::backend::frontend::FrontendPluginBackend;
use crate::backend::wasm::WasmPluginBackend;
use crate::error::PluginError;
use crate::types::{LoadedPlugin, PluginCallRequest, PluginCallResponse};

/// Backend interface implemented by concrete plugin runtimes.
///
/// Implementors decide whether they can execute a plugin via [`PluginBackend::accepts`],
/// and execute plugin calls via [`PluginBackend::call`].
#[async_trait]
pub trait PluginBackend: Send + Sync {
    fn accepts(&self, plugin: &LoadedPlugin) -> bool;

    async fn call(
        &self,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError>;
}

pub struct PluginRuntime {
    backends: Vec<Box<dyn PluginBackend>>,
}

impl PluginRuntime {
    pub fn new(backends: Vec<Box<dyn PluginBackend>>) -> Self {
        Self { backends }
    }

    pub async fn call(
        &self,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError> {
        // Backends are checked in order; the first backend that accepts the plugin
        // is the backend used to execute the call.
        for backend in &self.backends {
            if backend.accepts(plugin) {
                return backend.call(plugin, request).await;
            }
        }

        Err(PluginError::NoBackend { plugin_id: plugin.manifest.id.clone() })
    }

    pub fn with_default_backends() -> Self {
        Self::new(vec![Box::new(WasmPluginBackend::new()), Box::new(FrontendPluginBackend)])
    }

    pub fn with_api_base_url(api_base_url: String) -> Self {
        Self::new(vec![
            Box::new(WasmPluginBackend::new().with_api_base_url(api_base_url)),
            Box::new(FrontendPluginBackend),
        ])
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::with_default_backends()
    }
}
