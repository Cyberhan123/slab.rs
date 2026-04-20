# slab-runtime-macros

Procedural macros for Slab backend worker handlers.

## Role

`slab-runtime-macros` provides `#[backend_handler]`, an attribute macro used by
`bin/slab-runtime/src/infra/backends` worker implementations. The macro expands
annotated inherent impl blocks into `slab_runtime_core::backend` route tables
and `RuntimeWorkerHandler` implementations. When opted in as
`#[backend_handler(peer_bus = peer_bus)]`, it also generates typed peer-control
emitter helpers that route through `PeerControlBus`; the emitted helper set is
trimmed to the explicit `#[on_peer_control(...)]` variants declared in the impl,
with names following `emit_peer_<variant_snake>_deployment`,
`emit_peer_<variant_snake>_deployment_payload`, and
`emit_peer_<variant_snake>_generation`.

The generated paths intentionally target `::slab_runtime_core::backend::*`.
This crate is not a general runtime-domain macro package and should not be used
for application, transport, or task orchestration layers.

## Type

Rust proc-macro crate.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
