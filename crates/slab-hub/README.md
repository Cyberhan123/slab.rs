# slab-hub

Unified model hub abstraction for Slab.

## Role

`slab-hub` provides the shared hub client layer used by app-core model workflows.

- Exposes hub client, provider, endpoint, error, and download-progress abstractions.
- Centralizes model listing and download flows behind a provider interface.
- Supports feature-gated provider backends so builds can choose which hub integrations to ship.
- Keeps provider fallback and reachability handling out of higher-level business logic.

## Features

- `provider-hf-hub` enables the `hf-hub` backend.
- `provider-models-cat` enables the `models-cat` backend.
- `provider-huggingface-hub-rust` enables the alternate `huggingface-hub` backend.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).