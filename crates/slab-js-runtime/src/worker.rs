use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::{JsCallRequest, JsCallResponse, JsPluginPermissions};

#[derive(Clone)]
pub struct JsWorkerHandle {
    module_path: PathBuf,
    #[allow(dead_code)]
    permissions: JsPluginPermissions,
}

impl JsWorkerHandle {
    pub fn new(module_path: PathBuf, permissions: JsPluginPermissions) -> Self {
        Self { module_path, permissions }
    }

    pub async fn call(&self, request: JsCallRequest) -> Result<JsCallResponse> {
        if !self.module_path.is_file() {
            bail!("js module entry does not exist at {}", self.module_path.display());
        }

        if self.module_path != request.module_path {
            bail!(
                "js module path mismatch for plugin `{}`: expected {}, got {}",
                request.plugin_id,
                self.module_path.display(),
                request.module_path.display()
            );
        }

        bail!(
            "js runtime dispatch is not implemented for plugin `{}` in this build",
            request.plugin_id
        )
    }
}
