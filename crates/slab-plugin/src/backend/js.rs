use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use slab_js_runtime::{JsCallRequest, JsRuntime};

use crate::error::PluginError;
use crate::runtime::PluginBackend;
use crate::types::{LoadedPlugin, PluginCallRequest, PluginCallResponse};

pub struct JsPluginBackend {
    runtime: Arc<JsRuntime>,
}

impl JsPluginBackend {
    pub fn new(runtime: Arc<JsRuntime>) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl PluginBackend for JsPluginBackend {
    fn accepts(&self, plugin: &LoadedPlugin) -> bool {
        plugin.js_entry_path.is_some()
    }

    async fn call(
        &self,
        plugin: &LoadedPlugin,
        request: &PluginCallRequest,
    ) -> Result<PluginCallResponse, PluginError> {
        let Some(module_path) = plugin.js_entry_path.clone() else {
            return Err(PluginError::NoBackend { plugin_id: plugin.manifest.id.clone() });
        };

        let params = if request.input.trim().is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&request.input).map_err(|error| {
                PluginError::Runtime(format!(
                    "invalid js call params for `{}`: {error}",
                    plugin.manifest.id
                ))
            })?
        };

        let response = self
            .runtime
            .call(JsCallRequest {
                plugin_id: plugin.manifest.id.clone(),
                module_path,
                method: request.function.clone(),
                params,
                permissions: (&plugin.manifest.permissions).into(),
            })
            .await
            .map_err(PluginError::from)?;

        let output = serde_json::to_vec(&response.result).map_err(|error| {
            PluginError::Runtime(format!(
                "failed to serialize js result for `{}`: {error}",
                plugin.manifest.id
            ))
        })?;

        Ok(PluginCallResponse {
            output_text: String::from_utf8_lossy(&output).to_string(),
            output_base64: base64::engine::general_purpose::STANDARD.encode(output),
        })
    }
}
