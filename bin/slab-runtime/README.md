# slab-runtime

Standalone gRPC worker process for Slab inference backends.

## Role

`slab-runtime` is the model execution worker. It:

- Accepts gRPC requests from `bin/slab-server` over TCP or Unix IPC.
- Acts as the backend composition root for GGML, Candle, and ONNX runtime registrations, which now live in-package under `src/infra/backends/`.
- Uses `crates/slab-runtime-core` (package: `slab-runtime-core`) for backend worker protocol, admission, worker lifecycle helpers, and dispatch contracts.
- Organizes its own worker logic into `bootstrap/`, `api/`, `application/`, `domain/`, and `infra/`, with `src/main.rs` as a thin binary entrypoint.
- Runs as a separate OS process, isolating model memory and native library state from the HTTP gateway.

## Type

Rust package with a library core and a thin gRPC worker binary.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
