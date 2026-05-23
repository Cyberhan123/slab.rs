# slab-js-runtime

Sandboxed JavaScript runtime for slab plugins, powered by QuickJS (via `rquickjs`).

## Overview

This crate provides the embedded JS execution engine for plugins that declare
a `runtime.js.entry` in their `plugin.json`. It uses QuickJS through the
`rquickjs` Rust bindings, providing:

- ES2023 JavaScript execution in a sandboxed environment
- Host bridge (`globalThis.Slab`) for API requests and UI events
- Per-plugin worker isolation
- Permission-controlled access to slab APIs

## Architecture

```
slab-plugin (backend dispatcher)
  └─ JsPluginBackend
       └─ slab-js-runtime::JsRuntime
            └─ JsWorkerHandle (per plugin)
                 └─ rquickjs::Context (QuickJS VM)
                      └─ globalThis.Slab (host bridge)
```

Each plugin call creates a fresh QuickJS context with the Slab host bridge
injected, evaluates the plugin module, and calls the requested exported function.

## Host Bridge API

Available to plugins at `globalThis.Slab`:

- `Slab.pluginId` — the plugin's ID string
- `Slab.api.request(options)` — synchronous HTTP request to the slab API
- `Slab.ui.emit(topic, data)` — emit an event to the host UI

## Deno Compatibility

The `Slab.*` API is designed so that plugins can also run in Deno for local
development and testing. The embedded QuickJS engine does not provide Deno
APIs (like `Deno.readFile`), but the plugin-facing `Slab` bridge is identical.
