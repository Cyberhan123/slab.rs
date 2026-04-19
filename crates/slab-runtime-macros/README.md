# slab-runtime-macros

Procedural macros for Slab backend worker handlers.

## Role

`slab-runtime-macros` provides `#[backend_handler]`, an attribute macro used by
`bin/slab-runtime/src/infra/backends` worker implementations. The macro expands
annotated inherent impl blocks into `slab_runtime_core::backend` route tables
and `RuntimeWorkerHandler` implementations.

The generated paths intentionally target `::slab_runtime_core::backend::*`.
This crate is not a general runtime-domain macro package and should not be used
for application, transport, or task orchestration layers.

## Type

Rust proc-macro crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
