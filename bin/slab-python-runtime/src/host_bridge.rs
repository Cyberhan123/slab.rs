//! Host bridge module exposed to Python plugins as `import slab`.

use std::collections::HashMap;
use std::sync::Arc;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};
use serde_json::Value;
use slab_types::{PluginApiRequest, PluginRuntimeApiHostRequest, PluginRuntimeUiEmitRequest};
use tokio::runtime::Handle;

use crate::domain::RuntimeHost;

/// Inject the per-call `slab` module into `sys.modules` and the execution namespace.
pub fn inject(
    py: Python<'_>,
    plugin_id: &str,
    call_id: &str,
    host: Arc<dyn RuntimeHost>,
    runtime_handle: Handle,
    namespace: &Bound<'_, PyDict>,
) -> PyResult<()> {
    let api = Py::new(
        py,
        SlabApiBridge {
            plugin_id: plugin_id.to_owned(),
            call_id: call_id.to_owned(),
            host: host.clone(),
            runtime_handle: runtime_handle.clone(),
        },
    )?;
    let ui = Py::new(
        py,
        SlabUiBridge {
            plugin_id: plugin_id.to_owned(),
            call_id: call_id.to_owned(),
            host,
            runtime_handle,
        },
    )?;

    let slab = PyModule::new(py, "slab")?;
    slab.add("plugin_id", plugin_id)?;
    slab.add("api", api)?;
    slab.add("ui", ui)?;

    let sys = PyModule::import(py, "sys")?;
    let modules = sys.getattr("modules")?.cast_into::<PyDict>()?;
    modules.set_item("slab", &slab)?;
    namespace.set_item("slab", slab)?;
    Ok(())
}

#[pyclass]
struct SlabApiBridge {
    plugin_id: String,
    call_id: String,
    host: Arc<dyn RuntimeHost>,
    runtime_handle: Handle,
}

#[pymethods]
impl SlabApiBridge {
    fn client(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let bridge = PyModule::import(py, "slab_api_client.bridge")?;
        Ok(bridge.call_method0("create_client")?.unbind())
    }

    #[pyo3(signature = (method, path, headers=None, body=None, timeout_ms=None))]
    fn request(
        &self,
        py: Python<'_>,
        method: String,
        path: String,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
        timeout_ms: Option<u64>,
    ) -> PyResult<Py<PyAny>> {
        let request = PluginRuntimeApiHostRequest {
            call_id: self.call_id.clone(),
            plugin_id: self.plugin_id.clone(),
            request: PluginApiRequest {
                method,
                path,
                headers: headers.unwrap_or_default(),
                body,
                timeout_ms,
            },
        };
        let params = serde_json::to_value(request)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let response = host_request(&self.host, &self.runtime_handle, "slab.api.request", params)?;
        json_value_to_py(py, response)
    }
}

#[pyclass]
struct SlabUiBridge {
    plugin_id: String,
    call_id: String,
    host: Arc<dyn RuntimeHost>,
    runtime_handle: Handle,
}

#[pymethods]
impl SlabUiBridge {
    #[pyo3(signature = (topic, data=None))]
    fn emit(&self, py: Python<'_>, topic: String, data: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        let data = match data {
            Some(data) => py_to_json_value(py, data)?,
            None => Value::Null,
        };
        let request = PluginRuntimeUiEmitRequest {
            call_id: self.call_id.clone(),
            plugin_id: self.plugin_id.clone(),
            topic,
            data,
        };
        let params = serde_json::to_value(request)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        let response = host_request(&self.host, &self.runtime_handle, "slab.ui.emit", params)?;
        json_value_to_py(py, response)
    }
}

fn host_request(
    host: &Arc<dyn RuntimeHost>,
    runtime_handle: &Handle,
    method: &str,
    params: Value,
) -> PyResult<Value> {
    runtime_handle.block_on(host.request(method, params)).map_err(PyRuntimeError::new_err)
}

fn json_value_to_py(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    let json = PyModule::import(py, "json")?;
    let text = serde_json::to_string(&value)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(json.call_method1("loads", (text,))?.unbind())
}

fn py_to_json_value(py: Python<'_>, value: Py<PyAny>) -> PyResult<Value> {
    let json = PyModule::import(py, "json")?;
    let text: String = json.call_method1("dumps", (value,))?.extract()?;
    serde_json::from_str(&text).map_err(|error| PyValueError::new_err(error.to_string()))
}
