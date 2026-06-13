# slab-sandboxing

Sandbox policy types and platform drivers for Slab.

## Role

`slab-sandboxing` defines the sandbox contract used by command and plugin hosts:

- Sandbox policy, permission, network, and execution-mode types.
- Platform driver traits and setup status reporting.
- Pass-through behavior for unsupported or disabled sandbox modes.
- Linux, macOS, and Windows driver entrypoints.

This crate should stay focused on sandbox policy evaluation and process isolation primitives. Tool routing, plugin authorization, UI approvals, and API handlers belong in higher-level host crates.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-sandboxing
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
