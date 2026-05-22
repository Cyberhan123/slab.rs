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
        Self { workers: DashMap::new() }
    }

    pub async fn call(&self, req: JsCallRequest) -> Result<JsCallResponse> {
        match self.workers.entry(req.plugin_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(entry) => {
                let worker = entry.get().clone();
                worker.call(req).await
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let worker = JsWorkerHandle::new(req.module_path.clone(), req.permissions.clone());
                entry.insert(worker.clone());
                worker.call(req).await
            }
        }
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
