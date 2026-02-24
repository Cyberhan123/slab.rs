# Project Guidelines

## Code Style
- Rust: keep module boundaries explicit (`config`/`state`/`routes`/`middleware`), follow existing `thiserror` + typed error patterns (`slab-server/src/error.rs`, `slab-diffusion/src/error.rs`).
- Rust async: prefer Tokio-based async flows and existing runtime abstractions in `slab-core/src/runtime/` (pipeline/orchestrator) over ad-hoc task orchestration.
- TS/React: keep `slab-app/src/` minimal and command-driven; current UI is a thin Tauri invoke shell (`slab-app/src/App.tsx`).
- Do not move code across crates unless required; this workspace uses crate-level responsibility separation.

## Architecture
- `slab-server`: HTTP/API layer, middleware, persistence, task lifecycle (`slab-server/src/main.rs`, `slab-server/src/routes/`, `slab-server/src/entities/`).
- `slab-core`: unified runtime API + orchestrator/pipeline + backend abstractions (`slab-core/src/api/`, `slab-core/src/runtime/`, `slab-core/src/engine/`).
- `slab-llama` / `slab-whisper` / `slab-diffusion`: safe wrappers around model backends.
- `*-sys` crates are FFI bindings only (bindgen + generated symbols); keep unsafe details there (`slab-llama-sys`, `slab-whisper-sys`, `slab-diffusion-sys`).
- `slab-app/src-tauri` is desktop host; frontend is in `slab-app/src`.

## Build and Test
- Workspace:
  - `cargo build --workspace`
  - `cargo test --workspace`
- Server:
  - `cargo check -p slab-server`
  - `cargo run -p slab-server`
- Frontend/Tauri:
  - `cd slab-app`
  - `bun install`
  - `bun run dev`
  - `bun run tauri dev`
  - `bun run build`
- Note: `slab-app/src-tauri/tauri.conf.json` uses `bun run` for `beforeDevCommand`/`beforeBuildCommand`; keep scripts/tooling consistent when changing frontend build flow.

## Project Conventions
- Root workspace excludes frontend source (`Cargo.toml` has `exclude = ["slab-app/src"]`); avoid Rust tooling assumptions on TS sources.
- API shape convention: `/v1/*` for user-facing AI/task endpoints, `/admin/*` for management, `/health` for health checks (`slab-server/src/routes/`).
- Long-running operations should be tracked through task entities/routes rather than ad-hoc status endpoints (`slab-server/src/routes/v1/tasks.rs`, `slab-server/src/entities/task.rs`).
- Prefer extending existing route modules (`routes/v1/*`, `routes/admin/*`) over creating parallel API trees.

## Integration Points
- Axum + tower middleware for server HTTP concerns (`slab-server/src/routes/mod.rs`, `slab-server/src/middleware/`).
- SQLx + migrations for persistence (`slab-server/migrations/`, `slab-server/src/entities/`).
- Model and binary fetching flows touch both `hf-hub` and `slab-libfetch` (`slab-server/src/routes/v1/models.rs`, `slab-libfetch/src/`).
- Tauri command bridge lives in `slab-app/src-tauri/src/lib.rs`; frontend calls through `@tauri-apps/api`.

## Security
- Admin auth is optional unless `SLAB_ADMIN_TOKEN` is set; production changes must preserve or strengthen this behavior (`slab-server/src/middleware/auth.rs`, `slab-server/src/config.rs`).
- CORS defaults to allow-all unless `SLAB_CORS_ORIGINS` is configured; do not broaden exposure when editing router config (`slab-server/src/routes/mod.rs`).
- Swagger is enabled by default (`SLAB_ENABLE_SWAGGER`); keep production hardening implications explicit in related changes.
- Tauri CSP is currently `null` in `slab-app/src-tauri/tauri.conf.json`; treat web content/security changes as sensitive.
