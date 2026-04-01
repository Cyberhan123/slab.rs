# slab-types

Shared semantic types and contract definitions for the Slab workspace.

## Role

`slab-types` is the foundational contract crate consumed across all workspace members. It provides:

- Shared semantic types for inference settings, model specifications, and runtime configuration.
- JSON-schema-friendly type definitions for the plugin manifest and model catalog.
- Shared chat types (`ConversationMessage`, `ChatReasoningEffort`, `ChatVerbosity`, etc.) re-exported by domain crates.

This crate has no runtime dependencies on axum, SQL, or inference libraries.

## Type

Rust library crate (shared types / contracts).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
