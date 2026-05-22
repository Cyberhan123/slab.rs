# slab-types

Shared semantic types and contract definitions for the Slab workspace.

## Role

`slab-types` is the foundational contract crate consumed across workspace members. It provides:

- Shared semantic types for model specifications and runtime contracts.
- JSON-schema-friendly type definitions for the plugin manifest and model catalog.
- Shared chat types (`ConversationMessage`, `ChatReasoningEffort`, `ChatVerbosity`, etc.) re-exported by domain crates.

Settings document, PMID catalog, and launch configuration logic live in `crates/slab-config`.

This crate has no runtime dependencies on axum, SQL, configuration storage, or inference libraries.

## Type

Rust library crate (shared types / contracts).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
