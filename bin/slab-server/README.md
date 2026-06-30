# slab-server

HTTP gateway for the Slab inference stack, built with [axum](https://github.com/tokio-rs/axum).

## Role

`slab-server` is the thin HTTP layer that exposes the Slab API in headless mode. It uses `crates/slab-app-core` directly and keeps the HTTP gateway separate from the desktop host, which enables browser/mobile/remote clients and third-party integrations.

- Serves the `/v1` REST API (chat, models, audio, images, tasks, sessions, settings, setup, system, backend, agent, ffmpeg, video).
- Serves `/v1/workspace/lsp/{language}` WebSocket sessions for workspace LSP JSON-RPC forwarding.
- Serves `/v1/plugins/rpc` for plugin JSON-RPC calls and `/v1/plugins/events` for plugin UI event fan-out.
- Publishes an OpenAPI schema at `/api-docs/openapi.json`.
- Delegates all business logic to `crates/slab-app-core`; it adds only axum `FromRef` extractors (`state_extractors.rs`) and HTTP error mapping (`error.rs`).
- Launches and monitors `bin/slab-runtime` through the shared `crates/slab-app-core::runtime_supervisor` using a `tokio::process` adapter.
- Owns the supervised `bin/slab-js-runtime` and `bin/slab-python-runtime` sidecars through `crates/slab-app-core` for JS and Python plugin calls.
- Proxies inference requests to `bin/slab-runtime` via gRPC through `GrpcGateway`, while keeping the HTTP host alive if an individual backend runtime crashes and needs to restart.

## Type

Rust binary (axum HTTP server).

## Local Validation

```sh
cargo test -p slab-server
cargo check -p slab-server
```

## Boundaries

- Keep HTTP routing and error mapping here; business logic belongs in `crates/slab-app-core`.
- Keep Tauri host-only behavior in `bin/slab-app/src-tauri`.
- File logging for `--log-file` uses a redacting size-rotating writer: 50 MiB per file, 5 files total, with secret-shaped values masked before disk writes.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
