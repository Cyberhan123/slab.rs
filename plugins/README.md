# Slab Plugins Workspace

Each plugin lives under:

`plugins/<plugin-id>/`

Required files:

- `plugin.json`
- `ui/index.html` (or your configured `ui.entry`)
- `wasm/plugin.wasm` (or your configured `wasm.entry`, optional for WebView-only plugins)

`plugin.json` shape:

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

For lightweight WebView-only plugins, omit the `wasm` field and only include
the UI assets in `integrity.filesSha256`.
