# slab-app-core

HTTP-free business logic library for Slab.

## Role

`slab-app-core` is the shared domain layer consumed by `bin/slab-server`. It contains:

- `context/` - application context and dependency wiring.
- `domain/` - domain models and service logic.
- `infra/` - database access, file storage, and external integrations.
- `config` - configuration loading and validation.
- `model_auto_unload` - automatic model eviction to manage memory.
- `schemas/` - shared request/response DTO types used by HTTP consumers.

SQLx migrations live in `migrations/`.

## Type

Rust library crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
