# slab-git

Git helpers for Slab agent and workspace integrations.

## Role

`slab-git` owns reusable Git status, diff, and commit helpers that operate on an explicit repository root.

It does not push, contact remote services, or implement UI-specific workspace behavior.

## Type

Rust library crate.

## Testing

- Run the crate test suite with `cargo test -p slab-git` from the repo root.

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
