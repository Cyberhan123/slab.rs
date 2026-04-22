# Slab Plugins Workspace

Each plugin lives under:

`plugins/<plugin-id>/`

Required files:

- `plugin.json`
- `ui/index.html` or the configured `runtime.ui.entry`
- any schema/assets referenced by `integrity.filesSha256`
- `wasm/plugin.wasm` or the configured `runtime.wasm.entry` when the plugin exposes WASM functions

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
    "slabApi": ["tasks:create", "tasks:read"],
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
    ]
  }
}
```

The first supported extension points are routes, sidebar entries, commands,
settings sections, and agent capabilities. Header, footer, chat toolbar, and
other shell slots are intentionally not open yet.

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
