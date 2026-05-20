# slab-file-system

Workspace-safe filesystem helpers for Slab agent tools.

## Role

`slab-file-system` owns reusable path resolution, workspace escape checks, basic read/write/list helpers, and unified diff patch application.

It is a pure helper crate and should not perform host notification, approval, or analytics work.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-file-system` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
