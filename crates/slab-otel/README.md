# slab-otel

OpenTelemetry and local telemetry helpers for Slab.

## Role

`slab-otel` is the shared telemetry integration crate. It owns:

- Typed OpenTelemetry settings and exporter configuration.
- Provider setup and tracing/log bridge installation helpers.
- Session telemetry helpers.
- GenAI semantic attribute helpers for messages, tools, finish reasons, and token metrics.
- W3C trace context parsing and injection helpers.

Host crates wire this crate into their startup and settings flows. Keep transport-specific API behavior, desktop setup, and business logic outside this crate.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-otel
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
