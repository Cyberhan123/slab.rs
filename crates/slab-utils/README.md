# slab-utils

Shared low-level utility crate for Slab.

## Role

`slab-utils` collects repo-wide helpers that are intentionally independent of product workflows:

- App home paths, settings/database/log/model/plugin directories, and runtime IPC paths.
- Atomic filesystem helpers, absolute path handling, JSON helpers, hashing, and library loading.
- PTY and process helpers used by workspace terminal flows.
- UDS compatibility helpers and Cargo/Bazel runfile resolution.
- Fuzzy matching, string truncation, and timing helpers.
- Windows installer payload helpers and sleep inhibition utilities.

Do not put HTTP handlers, Tauri commands, app-core business services, plugin policy decisions, or model-runtime orchestration in this crate.

## Type

Rust library crate.

## Testing

Run focused tests with:

```sh
cargo test -p slab-utils
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
