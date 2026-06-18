---
title: Model Manifest Schema
outline: deep
---

# Model Manifest Schema

Slab model packs use a `manifest.json` document at the root of the pack. The canonical JSON Schema is published at:

`https://slab.reorgix.com/manifests/v3/slab-manifest.schema.json`

Slab currently accepts model pack schema version 3 only. Older v1/v2 model packs are rejected during import.

## Local Pack Example

```json
{
  "$schema": "https://slab.reorgix.com/manifests/v3/slab-manifest.schema.json",
  "schema_version": 3,
  "deployment": "local",
  "id": "qwen2.5-0.5b-instruct",
  "label": "Qwen2.5 0.5B Instruct",
  "family": "llama",
  "context_window": 4096,
  "capabilities": ["text_generation", "chat_generation"],
  "engines": [{ "id": "ggml.llama", "format": "gguf" }],
  "sources": [
    {
      "kind": "hugging_face",
      "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
      "files": [{ "id": "Q8_0", "path": "Qwen2.5-0.5B-Instruct-Q8_0.gguf" }]
    }
  ],
  "variants": [
    { "id": "Q8_0", "label": "Q8_0", "$ref": "ref://variants/Q8_0.json" }
  ],
  "presets": [
    { "id": "default", "label": "Default", "$ref": "ref://presets/default.json" }
  ],
  "default_preset": "default"
}
```

## Cloud Pack Example

```json
{
  "$schema": "https://slab.reorgix.com/manifests/v3/slab-manifest.schema.json",
  "schema_version": 3,
  "deployment": "cloud",
  "id": "gpt_4_1_mini",
  "label": "GPT-4.1 mini",
  "family": "llama",
  "capabilities": ["text_generation", "chat_generation"],
  "context_window": 128000,
  "cloud": {
    "provider_id": "openai-main",
    "remote_model_id": "gpt-4.1-mini"
  }
}
```

## Notes

- The schema is generated from the Rust `slab-model-pack` crate with `schemars`.
- Local packs use `engines[]`, `sources[]`, `variants[]`, `presets[]`, and `default_preset`.
- Cloud packs use top-level `cloud { provider_id, remote_model_id, preferred_api_base?, credentials? }` and do not declare local runtime fields.
- Manifest entry references use `$ref`; variant documents own `$load_config`, and preset documents own `$inference_config`.
