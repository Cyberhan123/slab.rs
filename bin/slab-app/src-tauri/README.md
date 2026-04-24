# slab-app (Tauri backend)

Rust backend for the Slab desktop application, built with [Tauri v2](https://tauri.app/).

## Role

This crate is the native host process for the Slab desktop shell. It:

- Launches `bin/slab-server` as a local sidecar and waits for its HTTP health endpoint before the frontend starts issuing product API requests.
- Mounts local plugin webviews from the repo `plugins/` directory in development, and reads installed plugins from the writable app-data `plugins/` directory in packaged apps.
- Keeps product API traffic on HTTP; Tauri commands are reserved for host-only features such as plugin runtime integration.
- Enforces Tauri capability and permission boundaries defined in `tauri.conf.json`.

## Type

Rust binary (Tauri application host).

## License

AGPL-3.0-only. See [LICENSE](../../../LICENSE).
