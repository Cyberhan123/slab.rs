//! Host bridge module exposed to Python plugins as `import slab`.
//!
//! Injects a `slab` object into the plugin's execution namespace.  Plugins access
//! it simply by referencing `slab` (the object is pre-bound to the namespace
//! dict before the plugin source is executed, no actual `import` statement is
//! required or permitted).
//!
//! The bridge is implemented as Python code evaluated by the interpreter, which
//! keeps the PyO3 API surface minimal and avoids complex closure lifetimes.
//!
//! Available API inside plugins:
//! - `slab.plugin_id`  — the plugin's ID string
//! - `slab.api.request(method, path, *, headers=None, body=None, timeout_ms=None)` — HTTP to slab API
//! - `slab.ui.emit(topic, data=None)` — emit an event payload (returns JSON string)

use std::ffi::CString;

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Inject a `slab` binding into the execution namespace for the current plugin call.
///
/// After this call, the plugin source can reference `slab.plugin_id`,
/// `slab.api`, and `slab.ui` without any import statement.
pub fn inject(
    py: Python<'_>,
    plugin_id: &str,
    api_base: &str,
    namespace: &Bound<'_, PyDict>,
) -> PyResult<()> {
    // Sanitise values used inside the Python string (no injection risk: we
    // only need to produce valid Python string literals from Rust-controlled
    // strings that are already validated before reaching here).
    let plugin_id_py = format!("{plugin_id:?}"); // Rust Debug format → Python string literal
    let api_base_py = format!("{api_base:?}");

    let setup = format!(
        r#"import json
import urllib.request
import urllib.error
import time as _time

class _SlabApi:
    def __init__(self, base_url):
        self._base_url = base_url.rstrip('/')

    def request(self, method, path, headers=None, body=None, timeout_ms=None):
        if not path.startswith('/'):
            raise ValueError("slab.api.request: path must start with '/'")
        if '://' in path or path.startswith('//'):
            raise ValueError("slab.api.request: absolute URLs are not allowed")
        url = self._base_url + path
        timeout = min(timeout_ms or 15000, 60000) / 1000.0
        data = body.encode('utf-8') if body else None
        req = urllib.request.Request(url, data=data, headers=headers or {{}}, method=method)
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                return {{'status': resp.status, 'headers': dict(resp.headers), 'body': resp.read().decode('utf-8')}}
        except urllib.error.HTTPError as e:
            return {{'status': e.code, 'headers': dict(e.headers), 'body': e.read().decode('utf-8')}}

class _SlabUi:
    def __init__(self, plugin_id):
        self._plugin_id = plugin_id

    def emit(self, topic, data=None):
        return json.dumps({{
            'plugin_id': self._plugin_id,
            'topic': topic,
            'data': data,
            'ts': int(_time.time() * 1000),
        }})

class _Slab:
    def __init__(self, plugin_id, api_base_url):
        self.plugin_id = plugin_id
        self.api = _SlabApi(api_base_url)
        self.ui = _SlabUi(plugin_id)

slab = _Slab({plugin_id_py}, {api_base_py})
del _SlabApi, _SlabUi, _Slab
"#
    );

    let code = CString::new(setup.as_str())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{e}")))?;
    py.run(&code, Some(namespace), Some(namespace))?;
    Ok(())
}
