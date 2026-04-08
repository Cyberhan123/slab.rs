# slab-runtime

Standalone gRPC worker process for Slab inference backends.

## Role

`slab-runtime` is the model execution worker. It:

- Accepts gRPC requests from `bin/slab-server` over TCP or Unix IPC.
- Acts as the backend composition root for GGML, Candle, and ONNX runtime registrations.
- Uses `crates/slab-runtime-core` (package: `slab-runtime-core`) for runtime orchestration, scheduling, worker lifecycle, and dispatch contracts.
- Runs as a separate OS process, isolating model memory and native library state from the HTTP gateway.

## Type

Rust binary (gRPC server / inference worker).

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
