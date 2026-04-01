# slab-proto

Protobuf contract definitions for Slab server/runtime IPC.

## Role

`slab-proto` owns the `.proto` schema files and the generated Rust types used for gRPC communication between `bin/slab-server` and `bin/slab-runtime`. All cross-process message types for inference requests and responses are defined here.

## Type

Rust library crate (protobuf / gRPC contracts).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
