# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

**Workspace (Rust):**
- `cargo build --workspace` - Build all Rust crates
- `cargo test --workspace` - Run all tests
- `cargo check -p slab-server` - Quick check server compilation
- `cargo run -p slab-server` - Run the server

**Frontend (Tauri + React):**
- `cd slab-app && bun install` - Install dependencies
- `bun run dev` - Start Vite dev server
- `bun run tauri dev` - Run Tauri desktop app in dev mode
- `bun run build` - Production build
- `bun run api` - Generate TypeScript types from OpenAPI spec (requires running server)

**Testing:**
- Run single test: `cargo test --package <crate> -- <test_name>`
- Run with output: `cargo test -- --nocapture`

## Architecture Overview

This is a multi-language ML inference platform with a Rust backend and React/Tauri desktop frontend.

### Crate Structure

**`slab-server`** - HTTP/API layer
- Main entry: [`slab-server/src/main.rs`](slab-server/src/main.rs)
- Routes: [`slab-server/src/routes/`](slab-server/src/routes/) - `/v1/*` for AI endpoints, `/admin/*` for management, `/health`
- Middleware: CORS, trace ID injection, optional admin auth
- Entities: Task, model, session persistence with SQLx
- Error handling: Unified [`ServerError`](slab-server/src/error.rs) type with `thiserror`

**`slab-core`** - Unified runtime API
- Public API: [`slab-core/src/api/mod.rs`](slab-core/src/api/mod.rs) - facade for all runtime operations
- Orchestrator: [`slab-core/src/runtime/orchestrator.rs`](slab-core/src/runtime/orchestrator.rs) - task scheduling and lifecycle
- Pipeline: [`slab-core/src/runtime/pipeline.rs`](slab-core/src/runtime/pipeline.rs) - multi-stage computation (CPU pre/post-process + GPU)
- Backend abstractions: [`slab-core/src/runtime/backend/`](slab-core/src/runtime/backend/)
- Storage: [`slab-core/src/runtime/storage.rs`](slab-core/src/runtime/storage.rs) - task status and result tracking

**Model Backends** (safe wrappers):
- `slab-llama` - Llama text generation via GGML
- `slab-whisper` - Speech-to-text via Whisper
- `slab-diffusion` - Image generation via Stable Diffusion

**FFI Bindings** (`*-sys` crates):
- `slab-llama-sys`, `slab-whisper-sys`, `slab-diffusion-sys`
- Generated via bindgen - keep unsafe details contained here

**`slab-app`** - Desktop frontend
- `slab-app/src-tauri/` - Tauri Rust host (currently minimal)
- `slab-app/src/` - React + Vite app with TanStack Query
- Routes defined in [`slab-app/src/routes/index.tsx`](slab-app/src/routes/index.tsx)
- API types generated from OpenAPI: `src/lib/api/v1.d.ts`

### Key Patterns

**Error Handling:**
- Rust: Use `thiserror` for typed errors following the pattern in [`slab-server/src/error.rs`](slab-server/src/error.rs)
- Internal errors logged with full detail but only generic messages returned to clients (security)

**Async Runtime:**
- All async code uses Tokio
- Prefer existing runtime abstractions in [`slab-core/src/runtime/`](slab-core/src/runtime/) over ad-hoc task spawning

**Task Lifecycle:**
- Long-running operations tracked through task entities in [`slab-server/src/routes/v1/tasks.rs`](slab-server/src/routes/v1/tasks.rs)
- Task statuses: Pending, Running, Succeeded, Failed, Cancelled, SucceededStreaming
- Use `slab_core::api::backend()` to interact with AI backends

## Configuration

**Environment Variables:**
- `SLAB_LOG` - Log level (tracing filter syntax)
- `SLAB_LOG_JSON` - Set for JSON logging (production)
- `SLAB_DATABASE_URL` - SQLite path
- `SLAB_BIND_ADDRESS` - HTTP server bind address
- `SLAB_TRANSPORT_MODE` - "http", "ipc", or "both"
- `SLAB_ADMIN_TOKEN` - Optional admin auth token
- `SLAB_CORS_ORIGINS` - CORS origins (default: allow-all)
- `SLAB_ENABLE_SWAGGER` - Enable/disable Swagger UI

**Backend Config:**
- `SLAB_QUEUE_CAPACITY` - Orchestrator queue size (default: 64)
- `SLAB_BACKEND_CAPACITY` - Max concurrent requests per backend (default: 4)
- `SLAB_LLAMA_LIB_DIR`, `SLAB_WHISPER_LIB_DIR`, `SLAB_DIFFUSION_LIB_DIR` - Paths to shared libraries

## Code Style Guidelines

**Rust:**
- Keep module boundaries explicit: `config`/`state`/`routes`/`middleware`/`entities`
- Follow existing `thiserror` + typed error patterns
- Do not move code across crates unless required - workspace uses crate-level responsibility separation

**TypeScript/React:**
- Keep `slab-app/src/` minimal and command-driven
- UI is a thin Tauri invoke shell
- Use TanStack Query for server state
- Use openapi-fetch for API calls with types generated from OpenAPI

## OpenAPI Workflow

1. Server uses utoipa for OpenAPI annotations (see route handlers)
2. Run server: `cargo run -p slab-server`
3. Generate types: `cd slab-app && bun run api`
4. Frontend uses `openapi-fetch` and `openapi-react-query` with generated `v1.d.ts`

## Important Notes

- Root workspace excludes frontend source from Cargo (`exclude = ["slab-app/src"]` in root Cargo.toml)
- `slab-app/src-tauri/tauri.conf.json` uses `bun run` for build commands - keep scripts consistent
- Admin auth is optional unless `SLAB_ADMIN_TOKEN` is set
- Swagger is enabled by default - disable in production with `SLAB_ENABLE_SWAGGER=false`
- CSP is currently `null` in Tauri config - treat web content/security changes as sensitive
- Prefer extending existing route modules (`routes/v1/*`, `routes/admin/*`) over creating parallel API trees
