# slab-agent-tools

Built-in tool adapters for `slab-agent`.

## Role

`slab-agent-tools` contains host-provided deterministic tools and registration helpers for the Slab agent runtime.

- `slab-agent` keeps the orchestration kernel, tool traits, and routing abstractions.
- `slab-agent-tools` owns concrete built-in tool implementations and the helper that registers them with a `ToolRouter`.
- Host layers can depend on this crate without moving storage, transport, or business logic into `slab-agent`.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-agent-tools` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).