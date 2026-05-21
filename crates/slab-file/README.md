# slab-file

Workspace-safe file helpers for Slab agent and workspace integrations.

## Role

`slab-file` owns reusable path resolution, workspace escape checks, basic read/write/list helpers, unified diff patch application, gitignore-aware file search, and file watcher primitives.

It is a pure helper crate and should not perform host notification, approval, or analytics work.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-file` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
