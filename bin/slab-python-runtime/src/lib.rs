mod host_bridge;
mod interpreter;
mod permissions;
mod vfs;
mod worker;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use dashmap::DashMap;
use serde_json::Value;
use slab_types::DESKTOP_API_ORIGIN;

pub use permissions::PythonPluginPermissions;
pub use vfs::EmbeddedStdlib;
use worker::PythonWorkerHandle;

/// Configuration for the Python runtime's host environment.
#[derive(Clone)]
pub struct PythonRuntimeConfig {
    /// Base URL for the slab HTTP API (e.g. `http://127.0.0.1:3000`).
    pub api_base_url: String,
}

impl Default for PythonRuntimeConfig {
    fn default() -> Self {
        Self { api_base_url: DESKTOP_API_ORIGIN.to_owned() }
    }
}

/// The top-level Python plugin runtime managing per-plugin workers.
pub struct PythonRuntime {
    workers: DashMap<String, Arc<PythonWorkerHandle>>,
    config: PythonRuntimeConfig,
}

#[derive(Clone)]
pub struct PythonCallRequest {
    pub plugin_id: String,
    pub module_path: PathBuf,
    pub function: String,
    pub params: Value,
    pub permissions: PythonPluginPermissions,
}

pub struct PythonCallResponse {
    pub result: Value,
}

impl PythonRuntime {
    pub fn new() -> Self {
        Self { workers: DashMap::new(), config: PythonRuntimeConfig::default() }
    }

    pub fn with_config(config: PythonRuntimeConfig) -> Self {
        Self { workers: DashMap::new(), config }
    }

    pub async fn call(&self, req: PythonCallRequest) -> Result<PythonCallResponse> {
        let worker = match self.workers.entry(req.plugin_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(entry) => entry.get().clone(),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let handle = Arc::new(PythonWorkerHandle::new(
                    req.module_path.clone(),
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

impl Default for PythonRuntime {
    fn default() -> Self {
        Self::new()
    }
}
