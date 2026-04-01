# slab-core

Runtime orchestration library for Slab (package name: `slab-runtime-core`).

## Role

`slab-core` provides the core runtime layer used by `bin/slab-runtime`. It contains:

- Runtime builder and lifecycle management.
- Scheduler and request dispatch logic.
- Engine adapter traits and implementations for ggml-based backends (llama, whisper, diffusion).

HTTP and SQL concerns are intentionally excluded from this crate; they belong in `bin/slab-server` or `crates/slab-app-core`.

## Type

Rust library crate (runtime orchestration).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
