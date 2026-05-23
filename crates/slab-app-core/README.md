# slab-app-core

HTTP-free business logic library for Slab.

## Role

`slab-app-core` is the shared domain layer consumed by `bin/slab-server`. It contains:

- `context/` - application context and dependency wiring.
- `domain/` - domain models and service logic.
- `infra/` - database access, file storage, and external integrations.
- `config` / `launch` - thin re-exports of shared settings and launch helpers from `crates/slab-config`.
- `model_auto_unload` - automatic model eviction to manage memory.
- `schemas/` - shared request/response DTO types used by HTTP consumers.

Workspace LSP provider resolution, workspace-root validation, and language-server process spawning live here so `bin/slab-server` can stay limited to HTTP/WebSocket routing. Built-in providers resolve first from the workspace `node_modules/.bin`, then packaged `resources/libs/language-servers/{bin,node_modules/.bin}`, plugin server folders, and finally `PATH`.

JS plugin runtime gateway/client logic also lives here. `PluginService` dispatches JS plugin calls to the supervised `bin/slab-js-runtime` sidecar over stdio JSON-RPC, while `crates/slab-plugin` remains registry/WASM/frontend focused and does not depend on Deno implementation details.

Settings document ownership, PMID catalog behavior, settings file migration, host config defaults, and runtime launch resolution live in `crates/slab-config`. `slab-app-core` adapts that logic to app services and existing storage only.

SQLx migrations live in `migrations/`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
