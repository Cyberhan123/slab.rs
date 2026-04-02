# slab-app (Tauri backend)

Rust backend for the Slab desktop application, built with [Tauri v2](https://tauri.app/).

## Role

This crate is the native host process for the Slab desktop shell. It:

- Launches `bin/slab-runtime` through the shared `crates/slab-app-core::runtime_supervisor` using a Tauri sidecar adapter.
- Automatically restarts individual runtime backends after unexpected sidecar exits, while keeping graceful shutdown under desktop-host control.
- Mounts local plugin webviews from the `plugins/` directory.
- Exposes native IPC commands to the frontend via `bin/slab-app/src-tauri/src/api/`, which delegate to `crates/slab-app-core`.
- Enforces Tauri capability and permission boundaries defined in `tauri.conf.json`.

## Type

Rust binary (Tauri application host).

## License

AGPL-3.0-only. See [LICENSE](../../../LICENSE).
