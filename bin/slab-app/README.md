# @slab/app

Bun workspace wrapper for the Slab Tauri desktop application.

## Role

`bin/slab-app` owns the JavaScript package entrypoint for the Tauri shell. It exists so root scripts can invoke the Tauri CLI from a Bun workspace package while the Rust host implementation remains under `src-tauri/`.

The production desktop UI lives in `packages/slab-desktop`. The Tauri host, sidecar setup, permissions, capabilities, and CSP live in `bin/slab-app/src-tauri`.

## Type

Bun-managed application package.

## Commands

Prefer the repo-root scripts for normal development:

```sh
bun run dev:app
bun run build:app
bun run build:windows-installer
```

Use local scripts only when working directly on the Tauri package:

```sh
bun run --cwd bin/slab-app tauri dev
bun run --cwd bin/slab-app tauri build
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
