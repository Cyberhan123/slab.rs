//! Python plugin worker: runs a single plugin call inside CPython via PyO3.

use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use slab_types::{PluginRuntimeCallRequest, PluginRuntimeCallResponse};
use tokio::runtime::Handle;
use tracing::debug;

use crate::host_bridge;
use crate::{PythonRuntimeConfig, interpreter, security};

const PYTHON_EXECUTION_TIMEOUT: Duration = Duration::from_secs(30);

/// A handle to a Python plugin worker.
pub struct PythonWorkerHandle {
    module_path: PathBuf,
    config: Arc<PythonRuntimeConfig>,
}

impl PythonWorkerHandle {
    pub fn new(module_path: PathBuf, config: PythonRuntimeConfig) -> Result<Self> {
        if !module_path.is_file() {
            bail!("Python module entry does not exist at {}", module_path.display());
        }

        interpreter::init(config.embedded_stdlib.clone())?;
        debug!("created Python worker for module {}", module_path.display());

        Ok(Self { module_path, config: Arc::new(config) })
    }

    pub async fn call(
        &self,
        request: PluginRuntimeCallRequest,
        module_path: PathBuf,
    ) -> Result<PluginRuntimeCallResponse> {
        if self.module_path != module_path {
            bail!(
                "Python module path mismatch for plugin `{}`: expected {}, got {}",
                request.plugin_id,
                self.module_path.display(),
                module_path.display()
            );
        }

        let config = Arc::clone(&self.config);
        let runtime_handle = Handle::current();
        let result = tokio::task::spawn_blocking(move || {
            execute_python_call(&module_path, request, &config, runtime_handle)
        })
        .await??;

        Ok(PluginRuntimeCallResponse { result })
    }
}

fn execute_python_call(
    module_path: &Path,
    request: PluginRuntimeCallRequest,
    config: &PythonRuntimeConfig,
    runtime_handle: Handle,
) -> Result<serde_json::Value> {
    let source = std::fs::read_to_string(module_path)
        .map_err(|error| anyhow::anyhow!("failed to read plugin module: {error}"))?;
    let params_json = serde_json::to_string(&request.params)?;
    let timeout_secs = PYTHON_EXECUTION_TIMEOUT.as_secs();
    let origin = module_path.display().to_string();

    Python::attach(|py| {
        let namespace = PyDict::new(py);
        namespace.set_item("__name__", "__slab_plugin__")?;
        namespace.set_item("__file__", origin.as_str())?;
        namespace.set_item("__package__", "")?;

        host_bridge::inject(
            py,
            &request.plugin_id,
            &request.call_id,
            config.host.clone(),
            runtime_handle,
            &namespace,
        )
        .map_err(|error| anyhow::anyhow!("host bridge injection failed: {error}"))?;
        security::install(py).map_err(|error| anyhow::anyhow!("security setup failed: {error}"))?;

        let source_c = CString::new(source.as_str())
            .map_err(|error| anyhow::anyhow!("null byte in plugin source: {error}"))?;
        py.run(&source_c, Some(&namespace), Some(&namespace))
            .map_err(|error| anyhow::anyhow!("plugin execution error in {origin}: {error}"))?;

        let func = namespace
            .get_item(&request.export_name)
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .ok_or_else(|| {
                anyhow::anyhow!("plugin does not export function `{}`", request.export_name)
            })?;

        let call_code = call_wrapper_code(timeout_secs, cfg!(unix));
        let call_code_c = CString::new(call_code.as_str())
            .map_err(|error| anyhow::anyhow!("null byte in call wrapper: {error}"))?;
        namespace.set_item("_slab_fn", &func)?;
        namespace.set_item("_slab_params_json", &params_json)?;
        py.run(&call_code_c, Some(&namespace), Some(&namespace))
            .map_err(|error| anyhow::anyhow!("plugin call error in {origin}: {error}"))?;

        let result_json: String = namespace
            .get_item("_slab_result_json")
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .ok_or_else(|| anyhow::anyhow!("plugin function returned no result"))?
            .extract()
            .map_err(|error| anyhow::anyhow!("result extraction error: {error}"))?;

        serde_json::from_str(&result_json)
            .map_err(|error| anyhow::anyhow!("failed to parse plugin result as JSON: {error}"))
    })
}

fn call_wrapper_code(timeout_secs: u64, use_sigalrm: bool) -> String {
    let guarded_call = if use_sigalrm {
        format!(
            r#"import threading

if threading.current_thread() is threading.main_thread():
    import signal

    def _slab_timeout_handler(signum, frame):
        raise TimeoutError("Python plugin timed out after {timeout_secs}s")

    _slab_old_handler = signal.signal(signal.SIGALRM, _slab_timeout_handler)
    signal.alarm({timeout_secs})
    try:
        _slab_result_value = _slab_call_plugin(_slab_fn, _slab_params_json)
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, _slab_old_handler)
else:
    _slab_result_value = _slab_call_plugin(_slab_fn, _slab_params_json)
"#
        )
    } else {
        "_slab_result_value = _slab_call_plugin(_slab_fn, _slab_params_json)\n".to_owned()
    };

    format!(
        r#"import json

def _slab_call_plugin(fn, params_json):
    params_value = json.loads(params_json)
    try:
        return fn(params_value)
    except TypeError as direct_error:
        try:
            return fn(params_json)
        except TypeError:
            raise direct_error

{guarded_call}
if isinstance(_slab_result_value, str):
    try:
        _slab_result_json = json.dumps(json.loads(_slab_result_value))
    except Exception:
        _slab_result_json = json.dumps(_slab_result_value)
else:
    _slab_result_json = json.dumps(_slab_result_value)
"#
    )
}
