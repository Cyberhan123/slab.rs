use std::path::{Path, PathBuf};

use crate::error::AppCoreError;
use crate::infra::process_supervisor::resolve_sibling_sidecar_exe;

pub fn resolve_js_runtime_exe(server_exe: &Path) -> Result<PathBuf, AppCoreError> {
    resolve_sibling_sidecar_exe(server_exe, "slab-js-runtime")
}

pub fn resolve_python_runtime_exe(server_exe: &Path) -> Result<PathBuf, AppCoreError> {
    resolve_sibling_sidecar_exe(server_exe, "slab-python-runtime")
}
