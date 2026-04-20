---
title: Settings Document Schema
outline: deep
---

# Settings Document Schema

The canonical JSON Schema for `SettingsDocumentV2` is published at:

`https://slab.reorgix.com/manifests/v1/settings-document.schema.json`

## Example

```json
{
  "$schema": "https://slab.reorgix.com/manifests/v1/settings-document.schema.json",
  "schema_version": 2,
  "logging": {
    "level": "info",
    "json": false
  },
  "runtime": {
    "mode": "managed_children",
    "transport": "ipc"
  },
  "providers": {
    "registry": []
  },
  "models": {
    "auto_unload": {
      "enabled": false,
      "idle_minutes": 10,
      "min_free_system_memory_bytes": 1073741824,
      "min_free_gpu_memory_bytes": 536870912,
      "max_pressure_evictions_per_load": 3
    }
  },
  "server": {
    "address": "127.0.0.1:3000",
    "swagger": {
      "enabled": true
    }
  }
}
```

## Notes

- This schema is generated from the Rust `slab-types` crate with `schemars`.
- The published schema describes the persisted settings document, not the internal settings UI metadata used by the app.
- `manifests/settings/settings-schema.json` remains an internal settings metadata artifact unless and until it is replaced by a true public JSON Schema.
