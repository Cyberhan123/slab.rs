# Project Guidelines

## Code Style
- Rust: keep module boundaries explicit (`config`/`state`/`routes`/`middleware`), follow existing `thiserror` + typed error patterns (`slab-server/src/error.rs`, `slab-diffusion/src/error.rs`).
- Rust async: prefer Tokio-based async flows and existing runtime abstractions in `slab-core/src/runtime/` (pipeline/orchestrator) over ad-hoc task orchestration.
- TS/React: keep `slab-app/src/` minimal and command-driven; current UI is a thin Tauri invoke shell (`slab-app/src/App.tsx`).
- Do not move code across crates unless required; this workspace uses crate-level responsibility separation.

# Project Guidelines

## Architecture Overview

The project uses a **supervisor-gateway + gRPC worker** pattern:

```
slab-server  (HTTP gateway + supervisor, default :3000)
  ├─ API routes  →  domain services  →  gRPC gateway  →  slab-runtime worker(s)
  └─ SQLx persistence (SQLite)

slab-runtime  (standalone gRPC worker process)
  └─ slab-core orchestrator + GGML backends (llama / whisper / diffusion)
```

- `slab-server` is a **supervisor**: it spawns `slab-runtime` child processes and proxies AI inference requests to them over gRPC/IPC. The server itself does **not** embed model backends.
- `slab-runtime` is a self-contained gRPC server. Each instance runs one or more backends (`--enabled-backends llama,whisper,diffusion`). Multiple instances can run in parallel (separate ports or IPC sockets).
- `slab-core` is a **library** used by `slab-runtime`; it must not depend on server-side concerns (HTTP, SQL).
- `slab-proto` contains gRPC protobuf definitions (IPC contract between server and runtime). Generated code lives under `slab-proto/src/lib.rs` via tonic-build.

### Crate Responsibilities

| Crate | Role |
|---|---|
| `slab-server` | HTTP/API gateway, supervisor, domain services, persistence |
| `slab-runtime` | gRPC server process, wraps slab-core |
| `slab-proto` | tonic/prost codegen from `proto/slab/ipc/v1/*.proto` |
| `slab-core` | Runtime API facade, orchestrator, pipeline, engine backends |
| `slab-core-macros` | Procedural macros for common patterns |
| `slab-llama` / `slab-whisper` / `slab-diffusion` | Safe FFI wrappers around model libraries |
| `slab-llama-sys` / `slab-whisper-sys` / `slab-diffusion-sys` | Bindgen-generated unsafe FFI only |
| `slab-libfetch` | Model/binary download from HuggingFace Hub |
| `slab-app` | Tauri desktop host + React/Vite frontend |

## Code Style

- **Rust**: keep module boundaries explicit. `slab-server` uses `config` / `context` / `api` / `domain` / `infra` layers; follow existing `thiserror` + typed error patterns (`slab-server/src/error.rs`, `slab-diffusion/src/error.rs`).
- **Rust async**: prefer Tokio-based async flows and existing runtime abstractions in `slab-core/src/runtime/` over ad-hoc task spawning.
- **Do not** embed backends directly in `slab-server`; AI calls must go through `GrpcGateway` → `slab-runtime` → `slab-core`.
- **Do not** add HTTP/SQL dependencies to `slab-core` or `slab-runtime`; keep the IPC boundary clean.
- Do not move code across crates unless required; this workspace uses crate-level responsibility separation.
- **TS/React**: React 19 + Vite + Tauri 2. Use TanStack Query v5 + openapi-fetch for server state; Zustand for client state; Ant Design X for AI-focused UI components.

## Key Files

- **Server entry**: `slab-server/src/main.rs` — parses `SupervisorArgs`, spawns runtime workers, starts HTTP server.
- **Server config**: `slab-server/src/config.rs` — all `SLAB_*` env vars.
- **HTTP router**: `slab-server/src/api/mod.rs` + `slab-server/src/api/v1/mod.rs`.
- **Domain services**: `slab-server/src/domain/services/` — 11 services (`audio`, `backend`, `chat`, `config`, `ffmpeg`, `image`, `model`, `session`, `system`, `task`, `video`).
- **gRPC gateway**: `slab-server/src/infra/rpc/gateway.rs` — `GrpcGateway`, channel management with retry logic.
- **App state**: `slab-server/src/context/mod.rs` — `AppState`, `AppContext`, `ModelState`, `WorkerState`.
- **Runtime entry**: `slab-runtime/src/main.rs` — CLI args (`--grpc-bind`, `--enabled-backends`, `--shutdown-on-stdin-close`).
- **Runtime gRPC**: `slab-runtime/src/grpc/` — tonic service implementations, error mapping.
- **Core API**: `slab-core/src/api/mod.rs` — public facade `init()`, `backend()`, `CallBuilder`.
- **Protos**: `slab-proto/proto/slab/ipc/v1/` — `common.proto`, `llama.proto`, `whisper.proto`, `diffusion.proto`.
- **Tauri setup**: `slab-app/src-tauri/src/setup.rs` — sidecar launcher for `slab-server`.
- **Frontend routes**: `slab-app/src/routes/index.tsx`.

## Build and Test

**Workspace (Rust):**
```sh
cargo build --workspace
cargo test --workspace
cargo check -p slab-server
cargo check -p slab-runtime
cargo run -p slab-server
```

**Frontend / Tauri:**
```sh
cd slab-app
bun install
bun run dev          # Vite dev server only
bun run tauri dev    # Full Tauri desktop app
bun run build
bun run api          # Re-generate TS types from OpenAPI spec (server must be running)
```

**Proto re-generation**: `slab-proto` uses `tonic-build` in `build.rs`; proto changes are picked up automatically on `cargo build`.

**Note**: `slab-app/src-tauri/tauri.conf.json` uses `bun run` for `beforeDevCommand`/`beforeBuildCommand`; keep scripts/tooling consistent.

## Project Conventions

- **API shape**: `/v1/*` for AI/user-facing endpoints, `/health` for health checks (`slab-server/src/api/v1/`).
- Long-running AI operations are tracked through task entities (`slab-server/src/api/v1/tasks.rs`). Prefer task-based flows; avoid ad-hoc status endpoints.
- Extend existing route modules in `api/v1/` (chat, audio, images, video, models, session, config, backend, system, tasks, ffmpeg). Do not create parallel API trees.
- **gRPC transport**: supports HTTP (`http://host:port`) and IPC (`unix:///path` or Windows named pipe). Transport is controlled by `SLAB_TRANSPORT_MODE`.
- **Model auto-unload**: `slab-server/src/model_auto_unload.rs` runs a background task managing idle model eviction; do not bypass it for model lifecycle.
- **Database**: SQLx + SQLite. Migrations in `slab-server/migrations/`. Add new migrations with `YYYYMMDDHHmmSS_description.sql`; never modify existing ones.
- Root workspace excludes frontend source (`Cargo.toml` has `exclude = ["slab-app/src"]`); avoid Rust tooling assumptions on TS sources.

## Integration Points

- **HTTP layer**: Axum 0.8 + tower-http (CORS, trace, request-id) — `slab-server/src/api/`.
- **gRPC**: tonic 0.12 + parity-tokio-ipc for IPC transport — `slab-server/src/infra/rpc/`, `slab-runtime/src/grpc/`.
- **Persistence**: SQLx 0.8 + SQLite — `slab-server/src/infra/db/`, `slab-server/migrations/`.
- **Model fetch**: `slab-libfetch` + `hf-hub` — `slab-server/src/domain/services/model.rs`.
- **OpenAPI**: utoipa 5 + utoipa-swagger-ui — route handlers use `#[utoipa::path]`; aggregated in `v1::api_docs()`.
- **System monitoring**: `all-smi` crate — `SystemService` in `slab-server/src/domain/services/system.rs`.
- **Tauri bridge**: Tauri commands in `slab-app/src-tauri/src/lib.rs`; `setup.rs` launches `slab-server` as a sidecar binary.
- **Frontend API client**: openapi-fetch + openapi-react-query with types generated into `slab-app/src/lib/api/v1.d.ts`.

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `SLAB_BIND` | `localhost:3000` | HTTP server address |
| `SLAB_DATABASE_URL` | `sqlite://slab.db?mode=rwc` | SQLite path |
| `SLAB_LOG` | `info` | Log level (tracing filter syntax) |
| `SLAB_LOG_JSON` | unset | Enable JSON logging |
| `SLAB_TRANSPORT_MODE` | `http` | `http` or `ipc` for gRPC transport |
| `SLAB_LLAMA_GRPC_ENDPOINT` | auto | Llama runtime gRPC address |
| `SLAB_WHISPER_GRPC_ENDPOINT` | auto | Whisper runtime gRPC address |
| `SLAB_DIFFUSION_GRPC_ENDPOINT` | auto | Diffusion runtime gRPC address |
| `SLAB_LIB_DIR` | unset | Path to model shared libraries |
| `SLAB_SESSION_STATE_DIR` | `./tmp/slab-sessions` | Chat session storage |
| `SLAB_ADMIN_TOKEN` | unset | Optional admin auth token |
| `SLAB_CORS_ORIGINS` | allow-all | Restrict CORS origins |
| `SLAB_ENABLE_SWAGGER` | `true` | Toggle Swagger UI |
| `SLAB_QUEUE_CAPACITY` | `64` | Orchestrator queue size (runtime) |
| `SLAB_BACKEND_CAPACITY` | `4` | Max concurrent requests per backend (runtime) |

## Security

- Admin auth is optional unless `SLAB_ADMIN_TOKEN` is set; production changes must preserve or strengthen this behavior (`slab-server/src/api/middleware/`, `slab-server/src/config.rs`).
- CORS defaults to allow-all unless `SLAB_CORS_ORIGINS` is configured; do not broaden exposure when editing router config.
- Swagger is enabled by default; disable in production with `SLAB_ENABLE_SWAGGER=false`.
- Tauri CSP is currently `null` in `slab-app/src-tauri/tauri.conf.json`; treat web content/security changes as sensitive.
- gRPC endpoints (slab-runtime) should only be bound to localhost; never expose them externally without authentication.
