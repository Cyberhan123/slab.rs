mod host_ops;
mod permissions;
mod worker;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use serde_json::Value;
use slab_types::DESKTOP_API_ORIGIN;

pub use permissions::JsPluginPermissions;
use worker::JsWorkerHandle;

/// Configuration for the JS runtime's host environment.
#[derive(Clone)]
pub struct JsRuntimeConfig {
    /// Base URL for the slab HTTP API (e.g. `http://127.0.0.1:3000`).
    pub api_base_url: String,
}

impl Default for JsRuntimeConfig {
    fn default() -> Self {
        Self { api_base_url: DESKTOP_API_ORIGIN.to_owned() }
    }
}

/// The top-level JS plugin runtime managing per-plugin workers.
pub struct JsRuntime {
    workers: DashMap<String, Arc<JsWorkerHandle>>,
    config: JsRuntimeConfig,
}

#[derive(Clone)]
pub struct JsCallRequest {
    pub plugin_id: String,
    pub module_path: PathBuf,
    pub method: String,
    pub params: Value,
    pub permissions: JsPluginPermissions,
}

pub struct JsCallResponse {
    pub result: Value,
}

impl JsRuntime {
    pub fn new() -> Self {
        Self { workers: DashMap::new(), config: JsRuntimeConfig::default() }
    }

    pub fn with_config(config: JsRuntimeConfig) -> Self {
        Self { workers: DashMap::new(), config }
    }

    pub async fn call(&self, req: JsCallRequest) -> Result<JsCallResponse> {
        let worker = match self.workers.entry(req.plugin_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(entry) => entry.get().clone(),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let handle = Arc::new(JsWorkerHandle::new(
                    req.module_path.clone(),
                    req.permissions.clone(),
                    self.config.clone(),
                )?);
                entry.insert(handle.clone());
                handle
            }
        };
        worker.call(req).await
    }

    pub fn unload(&self, plugin_id: &str) {
        self.workers.remove(plugin_id);
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new()
    }
}
