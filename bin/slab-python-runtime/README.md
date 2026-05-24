# slab-python-runtime

Sandboxed Python plugin backend for Slab, powered by CPython through PyO3.

## Role

This binary is a supervised stdio sidecar used by `crates/slab-app-core` when a
plugin declares `runtime.python.entry` in `plugin.json`. It speaks
line-delimited JSON-RPC 2.0:

- `runtime.ready` is sent on startup.
- `plugin.call` receives `PluginRuntimeCallRequest`.
- The response is `PluginRuntimeCallResponse`.
- `slab.api.request` and `slab.ui.emit` are runtime-to-host callbacks; the
  runtime does not call the Slab HTTP API directly.

## Plugin module format

`runtime.python.entry` must point to a `.py` file inside the plugin package.
The exported function receives a JSON value and returns a JSON-serializable
value:

```python
import slab

def summarize(params):
    models = slab.api.request("GET", "/v1/models")
    slab.ui.emit("summary.finished", {"modelCount": len(models["body"])})
    return {"status": models["status"], "input": params}
```

For compatibility during the transition, functions that accept a JSON string
and return a JSON string are still supported. New plugins should use direct
JSON values.

## Embedded stdlib

`build.rs` generates an embedded pure-Python stdlib table from
`SLAB_PYTHON_STDLIB_DIR`, `PYO3_PYTHON`, or the local `python`/`python3`/`py -3`
installation. At runtime a custom `importlib.abc.MetaPathFinder` and `Loader`
are registered at `sys.meta_path[0]`, so embedded modules and packages load
before the real filesystem.

The VFS supports package `__init__.py`, module origins, package
`submodule_search_locations`, and cache invalidation hooks.

## Security model

The first version supports plugin `.py` files and pure Python stdlib modules
only. It does not support third-party wheels, native extensions, pip installs,
or plugin-provided package trees.

Ambient file, network, subprocess, and `ctypes` access is blocked with Python
audit hooks and restricted builtins. Slab API access and UI events must go
through the `slab` host bridge so app-core can re-authorize each callback.

## Type

Rust binary and library.
