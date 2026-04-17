---
title: Model Manifest Schema
outline: deep
---

# Model Manifest Schema

Slab model packs use a manifest document at the root of the pack. The canonical JSON Schema is published at:

`https://slab.reorgix.com/manifests/v1/slab-manifest.schema.json`

## Example

```json
{
  "$schema": "https://slab.reorgix.com/manifests/v1/slab-manifest.schema.json",
  "version": 1,
  "id": "openrouter-llama-3_1-8b-instruct",
  "label": "Llama 3.1 8B Instruct (OpenRouter)",
  "status": "ready",
  "family": "llama",
  "capabilities": ["text_generation"],
  "context_window": 131072,
  "runtime_presets": {
    "max_tokens": 81920,
    "temperature": 0.7,
    "top_p": 0.95,
    "top_k": 20,
    "min_p": 0.0,
    "presence_penalty": 1.5,
    "repetition_penalty": 1.0
  },
  "sources": [
    {
      "kind": "cloud",
      "provider_id": "openrouter-main",
      "remote_model_id": "meta-llama/llama-3.1-8b-instruct",
      "priority": 0
    }
  ]
}
```

## Notes

- The schema is generated from the Rust `slab-model-pack` crate with `schemars`.
- The public docs copy is the canonical published artifact for tooling and validation.
- Example manifests in the repo can still live under `manifests/`, but they should reference the public schema URL instead of a local relative schema path.
- `sources[]` entries serialize as flattened source objects. Legacy nested `source` wrappers are still accepted for backwards compatibility when reading older packs.
