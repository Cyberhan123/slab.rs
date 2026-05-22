mod permissions;
mod worker;

use std::path::PathBuf;

use anyhow::Result;
use dashmap::DashMap;
use serde_json::Value;

pub use permissions::JsPluginPermissions;
use worker::JsWorkerHandle;

pub struct JsRuntime {
    workers: DashMap<String, JsWorkerHandle>,
}

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
        Self { workers: DashMap::new() }
    }

    pub async fn call(&self, req: JsCallRequest) -> Result<JsCallResponse> {
        if let Some(worker) = self.workers.get(&req.plugin_id) {
            return worker.call(req).await;
        }

        let plugin_id = req.plugin_id.clone();
        let worker = JsWorkerHandle::new(req.module_path.clone(), req.permissions.clone());
        let response = worker.call(req).await?;
        self.workers.insert(plugin_id, worker);
        Ok(response)
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
