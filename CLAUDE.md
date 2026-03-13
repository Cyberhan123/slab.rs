# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Test Commands

**Quick development workflow:**
- `cargo make dev` - Full development build (runtime + server) and Tauri desktop app
- `cargo build --workspace` - Build all Rust crates
- `cargo test --workspace` - Run all tests
- `cargo check -p slab-server` - Quick check server compilation
- `cargo check -p slab-runtime` - Quick check runtime compilation
- `cargo run -p slab-server` - Run the HTTP gateway server
- `cargo run -p slab-runtime -- --help` - Runtime worker options

**Individual crates:**
- `cargo build -p slab-runtime` - Build gRPC worker binary
- `cargo build -p slab-server` - Build HTTP gateway
- `cargo test --package <crate> -- <test_name>` - Run single test
- `cargo test -- --nocapture` - Run tests with output

**Frontend (Tauri + React):**
- `cd slab-app && bun install` - Install dependencies
- `bun run dev` - Start Vite dev server only
- `bun run tauri dev` - Run full Tauri desktop app in dev mode
- `bun run build` - Production build
- `bun run api` - Generate TypeScript types from OpenAPI spec (requires running server)

## Architecture Overview

This is a **supervisor-gateway + gRPC worker** ML inference platform with a Rust backend and React/Tauri desktop frontend.

### Core Architecture Pattern

```
┌─────────────────┐     gRPC/IPC      ┌──────────────────┐
│  slab-server    │ ──────────────────>│  slab-runtime    │
│  (HTTP Gateway) │                    │  (Worker Process)│
│  + Supervisor   │                    │  - llama         │
└─────────────────┘                    │  - whisper       │
       │                                │  - diffusion     │
       v                                └──────────────────┘
  SQLite DB                                   │
       │                                        v
       │                                   slab-core
       v                            (Orchestrator + Pipeline)
  React/Tauri Frontend                         │
                                              v
                                        Model Backends
                                        (GGML/Whisper/SD)
```

**Key separation:**
- `slab-server` is a **supervisor** - spawns `slab-runtime` child processes and proxies AI inference requests over gRPC/IPC. Does NOT embed model backends directly.
- `slab-runtime` is a **standalone gRPC server** - runs one or more backends (llama/whisper/diffusion). Multiple instances can run in parallel on different ports or IPC sockets.
- `slab-core` is a **library** used by `slab-runtime` - must NOT depend on server-side concerns (HTTP, SQL).
- `slab-proto` contains gRPC contract definitions between server and runtime.

### Crate Structure

**`slab-server`** - HTTP/API Gateway + Supervisor
- Main entry: [`slab-server/src/main.rs`](slab-server/src/main.rs) - parses `SupervisorArgs`, spawns runtime workers, starts HTTP server
- Layered architecture:
  - `api/` - HTTP routes (`/v1/*`, `/health`), middleware, OpenAPI docs
  - `domain/` - Business logic, services, models (11 services: audio, backend, chat, config, ffmpeg, image, model, session, system, task, video)
  - `infra/` - Infrastructure (DB repositories, gRPC gateway/client, adapters)
  - `context/` - Application state (AppState, AppContext, ModelState, WorkerState)
- Entities: Task, Model, Session, Chat with SQLx persistence
- Error handling: Unified [`ServerError`](slab-server/src/error.rs) type with `thiserror`
- Config: [`slab-server/src/config.rs`](slab-server/src/config.rs) - all `SLAB_*` environment variables

**`slab-runtime`** - gRPC Worker Process
- Entry: [`slab-runtime/src/main.rs`](slab-runtime/src/main.rs) - CLI args for gRPC bind address, enabled backends, shutdown behavior
- gRPC service implementations in `slab-runtime/src/grpc/`
- Wraps `slab-core` as a library - exposes core functionality via gRPC endpoints
- Supports multiple backends per instance: `--enabled-backends llama,whisper,diffusion`

**`slab-core`** - Unified Runtime API Library
- Public API: [`slab-core/src/api/mod.rs`](slab-core/src/api/mod.rs) - facade for all runtime operations
- Orchestrator: [`slab-core/src/runtime/orchestrator.rs`](slab-core/src/runtime/orchestrator.rs) - task scheduling and lifecycle
- Pipeline: [`slab-core/src/runtime/pipeline.rs`](slab-core/src/runtime/pipeline.rs) - multi-stage computation (CPU pre/post-process + GPU)
- Backend abstractions: [`slab-core/src/runtime/backend/`](slab-core/src/runtime/backend/)
- Storage: [`slab-core/src/runtime/storage.rs`](slab-core/src/runtime/storage.rs) - task status and result tracking

**`slab-proto`** - gRPC Protocol Definitions
- Proto files: `slab-proto/proto/slab/ipc/v1/` - `common.proto`, `llama.proto`, `whisper.proto`, `diffusion.proto`
- Generated code: `slab-proto/src/lib.rs` via `tonic-build` in `build.rs`
- IPC contract between server and runtime - changes auto-picked up on `cargo build`

**Model Backends** (safe wrappers):
- `slab-llama` - Llama text generation via GGML
- `slab-whisper` - Speech-to-text via Whisper
- `slab-diffusion` - Image generation via Stable Diffusion

**FFI Bindings** (`*-sys` crates):
- `slab-llama-sys`, `slab-whisper-sys`, `slab-diffusion-sys`
- Generated via bindgen - keep unsafe details contained here

**`slab-app`** - Desktop Frontend
- `slab-app/src-tauri/` - Tauri Rust host with sidecar launcher ([`setup.rs`](slab-app/src-tauri/src/setup.rs) spawns `slab-server`)
- `slab-app/src/` - React + Vite app with TanStack Query v5 + Zustand + Ant Design X
- Routes: [`slab-app/src/routes/index.tsx`](slab-app/src/routes/index.tsx)
- API types generated from OpenAPI: `src/lib/api/v1.d.ts` via `openapi-fetch`

**Supporting crates:**
- `slab-libfetch` - Model/binary download from HuggingFace Hub
- `slab-core-macros` - Procedural macros for common patterns

### Key Patterns

**Architectural boundaries:**
- Do NOT embed backends directly in `slab-server`; AI calls must go through `GrpcGateway` → `slab-runtime` → `slab-core`
- Do NOT add HTTP/SQL dependencies to `slab-core` or `slab-runtime`; keep the IPC boundary clean
- Do not move code across crates unless required - workspace uses crate-level responsibility separation

**Error handling:**
- Rust: Use `thiserror` for typed errors following the pattern in [`slab-server/src/error.rs`](slab-server/src/error.rs)
- Internal errors logged with full detail but only generic messages returned to clients (security)

**Async runtime:**
- All async code uses Tokio
- Prefer existing runtime abstractions in [`slab-core/src/runtime/`](slab-core/src/runtime/) over ad-hoc task spawning

**Task lifecycle:**
- Long-running operations tracked through task entities in [`slab-server/src/api/v1/tasks/mod.rs`](slab-server/src/api/v1/tasks/mod.rs)
- Task statuses: Pending, Running, Succeeded, Failed, Cancelled, SucceededStreaming
- Use domain services in [`slab-server/src/domain/services/`](slab-server/src/domain/services/) instead of direct RPC calls

**gRPC transport:**
- Supports HTTP (`http://host:port`) and IPC (`unix:///path` or Windows named pipe)
- Controlled by `SLAB_TRANSPORT_MODE` env variable
- Gateway logic in [`slab-server/src/infra/rpc/gateway.rs`](slab-server/src/infra/rpc/gateway.rs)

## Configuration

**Environment Variables:**

| Variable | Default | Description |
|---|---|---|
| `SLAB_BIND` | `localhost:3000` | HTTP server bind address |
| `SLAB_DATABASE_URL` | `sqlite://slab.db?mode=rwc` | SQLite database path |
| `SLAB_LOG` | `info` | Log level (tracing filter syntax) |
| `SLAB_LOG_JSON` | unset | Enable JSON logging for production |
| `SLAB_TRANSPORT_MODE` | `http` | gRPC transport mode: `http`, `ipc`, or `both` |
| `SLAB_LLAMA_GRPC_ENDPOINT` | auto | Llama runtime gRPC address |
| `SLAB_WHISPER_GRPC_ENDPOINT` | auto | Whisper runtime gRPC address |
| `SLAB_DIFFUSION_GRPC_ENDPOINT` | auto | Diffusion runtime gRPC address |
| `SLAB_LIB_DIR` | unset | Path to model shared libraries |
| `SLAB_SESSION_STATE_DIR` | `./tmp/slab-sessions` | Chat session storage directory |
| `SLAB_ADMIN_TOKEN` | unset | Optional admin auth token |
| `SLAB_CORS_ORIGINS` | allow-all | CORS allowed origins |
| `SLAB_ENABLE_SWAGGER` | `true` | Enable/disable Swagger UI |
| `SLAB_QUEUE_CAPACITY` | `64` | Orchestrator queue size (runtime) |
| `SLAB_BACKEND_CAPACITY` | `4` | Max concurrent requests per backend (runtime) |

## Code Style Guidelines

**Rust:**
- Keep module boundaries explicit: `config`/`context`/`api`/`domain`/`infra` layers in server
- Follow existing `thiserror` + typed error patterns
- Extend existing route modules in `api/v1/` (chat, audio, images, video, models, session, config, backend, system, tasks, ffmpeg)
- Prefer extending existing modules over creating parallel API trees

**TypeScript/React:**
- Keep `slab-app/src/` minimal and command-driven
- UI is a thin Tauri invoke shell
- Use TanStack Query v5 for server state with `openapi-fetch`
- Use Zustand for client state
- Use Ant Design X for AI-focused UI components

## Database

- SQLx 0.8 + SQLite
- Migrations in `slab-server/migrations/`
- Add new migrations with format: `YYYYMMDDHHmmSS_description.sql`
- Never modify existing migrations

## OpenAPI Workflow

1. Server uses utoipa 5 for OpenAPI annotations (see route handlers with `#[utoipa::path]`)
2. Run server: `cargo run -p slab-server`
3. Generate types: `cd slab-app && bun run api`
4. Frontend uses `openapi-fetch` and `openapi-react-query` with generated `v1.d.ts`

## Security Considerations

- Admin auth is optional unless `SLAB_ADMIN_TOKEN` is set - production changes must preserve this behavior
- CORS defaults to allow-all unless `SLAB_CORS_ORIGINS` is configured - do not broaden exposure
- Swagger is enabled by default - disable in production with `SLAB_ENABLE_SWAGGER=false`
- Tauri CSP is currently `null` in `slab-app/src-tauri/tauri.conf.json` - treat web content/security changes as sensitive
- gRPC endpoints (slab-runtime) should only be bound to localhost - never expose externally without authentication

## Important Notes

- Root workspace excludes frontend source from Cargo (`exclude = ["slab-app/src"]` in root Cargo.toml)
- `slab-app/src-tauri/tauri.conf.json` uses `bun run` for build commands - keep scripts consistent
- Model auto-unload: `slab-server/src/model_auto_unload.rs` runs background task managing idle model eviction
- Proto changes are automatically picked up on rebuild via `tonic-build` in `slab-proto/build.rs`
- Tauri setup: [`slab-app/src-tauri/src/setup.rs`](slab-app/src-tauri/src/setup.rs) launches `slab-server` as sidecar binary
