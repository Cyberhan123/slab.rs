# Slab Plugins Workspace

Each plugin lives under:

`plugins/<plugin-id>/`

Required files:

- `plugin.json`
- `ui/index.html` or the configured `runtime.ui.entry`
- any schema/assets referenced by `integrity.filesSha256`
- `wasm/plugin.wasm` or the configured `runtime.wasm.entry` when the plugin exposes WASM functions
- `dist/plugin.js` or the configured `runtime.js.entry` when the plugin exposes JS backend functions

## Frontend build model

The default plugin UI runtime is a sandboxed Tauri child WebView. Do not use
Module Federation as the default third-party plugin model; reserve that for a
future trusted first-party path if the host can own the dependency graph.

Repository plugins can be built with Vite, React, and TypeScript. Use
`@slab/plugin-sdk` for host bridge calls and theme snapshots, and use
`@slab/plugin-ui` plus `@slab/plugin-ui/globals.css` for the stable plugin UI
ABI. `@slab/plugin-ui` intentionally exposes only a safe component subset
instead of the full `@slab/components` surface.

Run `bun run gen:plugin-packs` from the repo root to scan `plugins/*` for
directories that contain `plugin.json`, refresh `plugin.json`
`integrity.filesSha256` entries from the current plugin files, and emit
`.plugin.slab` archives to `plugins/dist/`.

Helper scripts now live under the repo-root `scripts/plugins/` directory.
Directories under `plugins/` without `plugin.json` are not treated as plugins.

## JS Backend Runtime

Plugins can expose backend functions by providing a `runtime.js.entry` in
`plugin.json`. The JS backend runs in an embedded QuickJS engine (via
`rquickjs`) with the following host bridge available at `globalThis.Slab`:

```javascript
// Available in all JS plugin backends:
Slab.pluginId      // string - the plugin's id
Slab.api.request({ method, path, headers, body })  // synchronous HTTP to slab API
Slab.ui.emit(topic, data)                          // emit event to host UI
```

### Plugin module format

Backend JS files use CommonJS module format:

```javascript
function myFunction(params) {
    // Use Slab.api.request() for host API calls
    var result = Slab.api.request({
        method: "POST",
        path: "/v1/chat/completions",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ messages: [{ role: "user", content: params.text }] })
    });
    return JSON.parse(result.body);
}

module.exports = { myFunction: myFunction };
```

### Deno compatibility

The embedded runtime uses CommonJS (`module.exports`) format. Plugins that
also need to run in Deno can use a simple build step (e.g. esbuild/rollup)
to bundle ES modules into a single CommonJS file, or use the CommonJS format
directly (which Deno supports via `--compat` or `require()` in Deno 2+).

The `Slab.*` API surface is identical between the embedded engine and a Deno
polyfill, so the plugin logic itself requires no modification across runtimes.

### Supported backends

| Backend | Engine | Use case |
|---------|--------|----------|
| WASM | Extism (Wasmtime) | High-performance, sandboxed, polyglot (Rust, Go, C, etc.) |
| JS | QuickJS (rquickjs) | Lightweight scripting, Deno-compatible API, rapid iteration |
| Frontend-only | Tauri WebView | UI-only plugins with no backend logic |

## Manifest v1

`plugin.json` v1 is the canonical declaration format. It separates runtime
assets from host-controlled extension points and agent-facing capabilities:

```json
{
  "manifestVersion": 1,
  "id": "example-plugin",
  "name": "Example Plugin",
  "version": "0.1.0",
  "compatibility": {
    "slab": ">=0.1.0",
    "pluginApi": "^1.0.0"
  },
  "runtime": {
    "ui": { "entry": "ui/index.html" },
    "wasm": { "entry": "wasm/plugin.wasm" }
  },
  "integrity": {
    "filesSha256": {
      "ui/index.html": "<sha256>",
      "wasm/plugin.wasm": "<sha256>",
      "schemas/input.schema.json": "<sha256>"
    }
  },
  "permissions": {
    "network": { "mode": "blocked", "allowHosts": [] },
    "ui": ["route:create", "sidebar:item:create", "command:create", "settings:section:create"],
    "agent": ["capability:declare", "mcpTool:expose"],
    "lsp": ["languageServer:declare"],
    "slabApi": ["models:read", "tasks:read"],
    "files": {
      "read": ["video"],
      "write": ["subtitle"]
    }
  },
  "contributes": {
    "routes": [
      { "id": "example.page", "path": "/plugins/example-plugin", "title": "Example" }
    ],
    "sidebar": [
      { "id": "example.nav", "label": "Example", "route": "example.page" }
    ],
    "commands": [
      { "id": "example.open", "label": "Open Example", "action": "openRoute", "route": "example.page" }
    ],
    "settings": [
      { "id": "example.settings", "title": "Example Settings", "schema": "schemas/settings.schema.json" }
    ],
    "agentCapabilities": [
      {
        "id": "example.run",
        "kind": "workflow",
        "description": "Run the example workflow.",
        "inputSchema": "schemas/input.schema.json",
        "transport": { "type": "pluginCall", "function": "run" },
        "exposeAsMcpTool": true
      }
    ],
    "languageServers": [
      {
        "id": "example.python",
        "languages": ["python"],
        "transport": {
          "type": "stdio",
          "command": "pyright-langserver",
          "args": ["--stdio"]
        }
      },
      {
        "id": "example.remote",
        "languages": ["rust"],
        "transport": {
          "type": "webSocket",
          "url": "ws://127.0.0.1:9257/lsp"
        }
      }
    ]
  }
}
```

The first supported extension points are routes, sidebar entries, commands,
settings sections, agent capabilities, and workspace language servers. Header,
footer, chat toolbar, and other shell slots are intentionally not open yet.

## Legacy manifests

Legacy manifests without `manifestVersion` are still accepted and normalized to
the v1 runtime/permissions shape:

```json
{
  "id": "example-plugin",
  "name": "Example Plugin",
  "version": "0.1.0",
  "ui": { "entry": "ui/index.html" },
  "wasm": { "entry": "wasm/plugin.wasm" },
  "integrity": {
    "filesSha256": {
      "ui/index.html": "<sha256>",
      "wasm/plugin.wasm": "<sha256>"
    }
  },
  "network": {
    "mode": "blocked",
    "allowHosts": []
  }
}
```
