# slab-python-runtime

Sandboxed Python plugin runtime for slab, powered by CPython via [PyO3](https://pyo3.rs).

## Overview

This crate provides the embedded Python execution engine for plugins that
declare a `runtime.python.entry` in their `plugin.json`.  It uses CPython
through PyO3 and delivers:

- **Static embedding** — Python standard-library `.py` files are bundled
  as `&'static [u8]` bytes inside the binary.  No `.py` files need to exist
  on the real filesystem at runtime.
- **Virtual-filesystem loader** — a custom `importlib.abc.MetaPathFinder` /
  `Loader` pair is registered at `sys.meta_path[0]` before any user code
  runs.  Embedded modules take priority over the real filesystem.
- **Host bridge** (`slab`) — injected into the plugin's execution namespace
  so plugins can call the slab HTTP API and emit UI events without any
  third-party import.
- **Per-plugin isolation** — each plugin worker gets a fresh `globals` /
  `locals` dict; state does not leak across calls.
- **Execution timeout** — SIGALRM-based (Unix) or best-effort (other
  platforms).

## Architecture

```
PythonRuntime
  └─ PythonWorkerHandle  (per plugin)
       └─ Python::with_gil
            ├─ interpreter::init  →  vfs::register  →  sys.meta_path[0]
            ├─ host_bridge::inject  →  slab object in locals
            └─ py.run(plugin_source)  →  fn(json_params) → json_result
```

## Static Embedding

Populate an `EmbeddedStdlib` before calling `interpreter::init`:

```rust
use slab_python_runtime::EmbeddedStdlib;

let mut stdlib = EmbeddedStdlib::default();
stdlib
    .add("mypackage.utils", include_bytes!("../python/mypackage/utils.py"))
    .add("mypackage.models", include_bytes!("../python/mypackage/models.py"));
```

The bytes are `include_bytes!` – they are linked directly into the binary,
so no `.py` files are needed at runtime.

## Virtual Filesystem

`vfs::register` injects the following Python code once at startup:

```python
class _EmbeddedFinder(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path, target=None):
        src = self._modules.get(fullname)
        if src is None:
            return None
        loader = _EmbeddedLoader(fullname, src)
        return importlib.machinery.ModuleSpec(fullname, loader, ...)

sys.meta_path.insert(0, _EmbeddedFinder(_slab_embedded_modules))
```

The `_slab_embedded_modules` dict is built from the `EmbeddedStdlib` map
on the Rust side and passed as a Python `dict[str, bytes]`.

## Host Bridge

Available to plugins at `slab` (no import statement needed):

```python
slab.plugin_id               # str — the plugin's ID
slab.api.request(            # synchronous HTTP to the slab API
    method, path,
    headers=None,
    body=None,
    timeout_ms=None,
) → {"status": int, "headers": dict, "body": str}

slab.ui.emit(topic, data=None)  # emit UI event; returns JSON string
```

## Plugin module format

```python
import json

def my_function(params_json):
    params = json.loads(params_json)
    result = slab.api.request("GET", "/v1/models")
    return json.dumps(result)
```

## Build requirements

CPython development headers must be installed:

```sh
# Ubuntu / Debian
sudo apt-get install python3-dev

# macOS
brew install python3
```
