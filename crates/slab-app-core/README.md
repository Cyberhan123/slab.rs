# slab-app-core

HTTP-free business logic library for Slab.

## Role

`slab-app-core` is the shared domain layer consumed by both `bin/slab-server` (HTTP path) and `bin/slab-app` (native Tauri IPC path). It contains:

- `context/` — application context and dependency wiring.
- `domain/` — domain models and service logic.
- `infra/` — database access, file storage, and external integrations.
- `config` — configuration loading and validation.
- `model_auto_unload` — automatic model eviction to manage memory.
- `schemas/` — shared request/response DTO types used by both HTTP and IPC consumers.
- `tauri_bridge` (feature-gated) — Tauri IPC command implementations that wrap domain services.

SQLx migrations live in `migrations/`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
