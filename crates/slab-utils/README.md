# slab-utils

Shared low-level utility crate for Slab.

## Role

`slab-utils` collects repo-wide helpers that are intentionally independent of product workflows:

- App home paths, settings/database/log/model/plugin directories, and runtime IPC paths.
- Atomic filesystem helpers, absolute path handling, JSON helpers, hashing, and library loading.
- PTY and process helpers used by workspace terminal flows.
- UDS compatibility helpers and Cargo binary resolution for tests (`cargo_bin`).
- Fuzzy matching, string truncation, and timing helpers. The context-budget truncation family lives here: `truncate_middle_bytes` (byte-budgeted head/tail split with an omission marker), `decode_truncated_head_tail` (lossy-decode command output then middle-truncate, keeping the tail readable), and `truncate_line_bytes` (cap one oversized line with a `[...line truncated, N bytes total]` marker).
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
