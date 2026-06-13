# slab-agent-tracing

Session-scoped agent trace logging for Slab.

## Role

`slab-agent-tracing` provides the small trace sink API used to record structured agent lifecycle events. It includes:

- Trace context and event payload types.
- A no-op sink for disabled tracing.
- A file sink for per-session JSONL logs.
- Helpers for stable session log names and paths.

It does not decide when tracing is enabled, where application logs live, or how telemetry is exported. Host configuration and lifecycle wiring belong in `crates/slab-app-core`, `crates/slab-config`, and `crates/slab-otel`.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-agent-tracing
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
