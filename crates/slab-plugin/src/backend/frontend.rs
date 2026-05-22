use async_trait::async_trait;

use crate::error::PluginError;
use crate::runtime::PluginBackend;
use crate::types::{LoadedPlugin, PluginCallRequest, PluginCallResponse};

pub struct FrontendPluginBackend;

#[async_trait]
impl PluginBackend for FrontendPluginBackend {
    fn accepts(&self, _plugin: &LoadedPlugin) -> bool {
        true
    }

    async fn call(
        &self,
        plugin: &LoadedPlugin,
        _request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError> {
        Err(PluginError::NotCallable { plugin_id: plugin.manifest.id.clone() })
    }
}
