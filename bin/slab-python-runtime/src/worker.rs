//! Python plugin worker: runs a single plugin call inside CPython via PyO3.
//!
//! Each call to [`PythonWorkerHandle::call`] spawns a blocking thread
//! (CPython's GIL is not `Send`), initialises a fresh `__dict__` namespace,
//! injects the `slab` host bridge, executes the plugin source, calls the
//! requested function, and returns the JSON-serialised result.

use std::ffi::CString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use tracing::debug;

use crate::host_bridge;
use crate::interpreter;
use crate::{PythonCallRequest, PythonCallResponse, PythonRuntimeConfig};

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

        // Ensure CPython is initialised once per process.  Subsequent calls to
        // `interpreter::init` are idempotent.
        interpreter::init(config.embedded_stdlib.clone())?;

        debug!("created Python worker for module {}", module_path.display());

        Ok(Self { module_path, config: Arc::new(config) })
    }

    pub async fn call(&self, request: PythonCallRequest) -> Result<PythonCallResponse> {
        if self.module_path != request.module_path {
            bail!(
                "Python module path mismatch for plugin `{}`: expected {}, got {}",
                request.plugin_id,
                self.module_path.display(),
                request.module_path.display()
            );
        }

        let module_path = self.module_path.clone();
        let function = request.function.clone();
        let params = request.params.clone();
        let plugin_id = request.plugin_id.clone();
        let config = Arc::clone(&self.config);

        let result = tokio::task::spawn_blocking(move || {
            execute_python_call(&module_path, &function, &params, &plugin_id, &config)
        })
        .await??;

        Ok(PythonCallResponse { result })
    }
}

fn execute_python_call(
    module_path: &PathBuf,
    function: &str,
    params: &Value,
    plugin_id: &str,
    config: &PythonRuntimeConfig,
) -> Result<Value> {
    let source = std::fs::read_to_string(module_path)
        .map_err(|e| anyhow::anyhow!("failed to read plugin module: {e}"))?;

    let params_json = serde_json::to_string(params)?;
    let timeout_secs = PYTHON_EXECUTION_TIMEOUT.as_secs();
    let origin = module_path.display().to_string();

    Python::with_gil(|py| {
        let namespace = PyDict::new(py);

        // Inject the slab host bridge so plugins can `import slab`.
        host_bridge::inject(py, plugin_id, &config.api_base_url, &namespace)
            .map_err(|e| anyhow::anyhow!("host bridge injection failed: {e}"))?;

        // Execute the plugin source in the prepared namespace.
        let source_c = CString::new(source.as_str())
            .map_err(|e| anyhow::anyhow!("null byte in plugin source: {e}"))?;
        py.run(&source_c, Some(&namespace), Some(&namespace))
            .map_err(|e| anyhow::anyhow!("plugin execution error in {origin}: {e}"))?;

        let func = namespace
            .get_item(function)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .ok_or_else(|| anyhow::anyhow!("plugin does not export function `{function}`"))?;

        // Run with a signal-based timeout via Python's `signal` module.
        let timeout_code = format!(
            r#"import threading
if threading.current_thread() is threading.main_thread():
    import signal

    def _slab_timeout_handler(signum, frame):
        raise TimeoutError("Python plugin timed out after {timeout_secs}s")

    _slab_old_handler = signal.signal(signal.SIGALRM, _slab_timeout_handler)
    signal.alarm({timeout_secs})
    try:
        _slab_result = _slab_fn(_slab_params)
    finally:
        signal.alarm(0)
        signal.signal(signal.SIGALRM, _slab_old_handler)
else:
    _slab_result = _slab_fn(_slab_params)
"#
        );

        namespace.set_item("_slab_fn", &func)?;
        namespace.set_item("_slab_params", &params_json)?;

        // SIGALRM is Unix-only; on other platforms fall back to a plain call.
        let result_json: String = if cfg!(unix) {
            let timeout_c = CString::new(timeout_code.as_str())
                .map_err(|e| anyhow::anyhow!("null byte in timeout code: {e}"))?;
            py.run(&timeout_c, Some(&namespace), Some(&namespace))
                .map_err(|e| anyhow::anyhow!("plugin execution error in {origin}: {e}"))?;
            namespace
                .get_item("_slab_result")
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .ok_or_else(|| anyhow::anyhow!("plugin function returned no result"))?
                .extract::<String>()
                .map_err(|e| anyhow::anyhow!("result extraction error: {e}"))?
        } else {
            let result = func
                .call1((params_json.as_str(),))
                .map_err(|e| anyhow::anyhow!("plugin call error in {origin}: {e}"))?;
            result
                .extract::<String>()
                .map_err(|e| anyhow::anyhow!("result extraction error: {e}"))?
        };

        serde_json::from_str(&result_json)
            .map_err(|e| anyhow::anyhow!("failed to parse plugin result as JSON: {e}"))
    })
}
