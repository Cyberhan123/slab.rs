# slab-server

HTTP gateway for the Slab inference stack, built with [axum](https://github.com/tokio-rs/axum).

## Role

`slab-server` is the thin HTTP layer that exposes the Slab API. It:

- Serves the `/v1` REST API (chat, models, audio, images, tasks, sessions, settings, setup, system, backend, agent, ffmpeg, video).
- Publishes an OpenAPI schema at `/api-docs/openapi.json`.
- Delegates all business logic to `crates/slab-app-core`; it adds only axum `FromRef` extractors (`state_extractors.rs`) and HTTP error mapping (`error.rs`).
- Proxies inference requests to `bin/slab-runtime` via gRPC through `GrpcGateway`.

## Type

Rust binary (axum HTTP server).

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
