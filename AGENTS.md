# Project Guidelines

## Agent Workflow

- Read this file before making changes.
- The `.agents/skills` directory contains operational skills, not generic app source.
- Open the relevant `.agents/skills/<skill>/SKILL.md` when the task matches that skill.
- When documentation and code disagree, trust the current code, then update the documentation.
- Keep AI infrastructure deterministic: example commands should work from the repo root, local references should be preferred over remote-only guidance, and skill triggers should match the actual stack in this repository.

## Project Skill Routing

Use these local project skills first. They are the slab.rs-specific adapter layer over the more generic imported skills.

- `slab-frontend-page`: page and route work in `slab-app`
- `slab-ui-primitives`: shared UI primitives, forms, and theme-level frontend work
- `slab-tauri-app`: `src-tauri`, capabilities, sidecars, plugins, and desktop security boundaries
- `slab-server-feature`: `slab-server` API, domain, context, infra, and migration work
- `slab-runtime-async`: `slab-core` and `slab-runtime` async orchestration work
- `slab-ui-review`: project-specific UI review

Read [docs/ai-skill-map.md](docs/ai-skill-map.md) for the routing map and related generic skills to consult through these project skills.

## Supporting Skills

Use these after selecting the matching project skill, or when the user explicitly names them.

- `frontend-design`: bold visual direction for page work in `slab-app`
- `ui-ux-pro-max`: design-system-first UI exploration and remediation ideas
- `use-x-chat`: `useXChat` and `useXConversations` work in `slab-app/src/pages/chat/**`
- `x-request`: `XRequest` transport changes in `slab-app/src/pages/chat/chat-context.ts`
- `x-markdown`: Markdown rendering changes in `slab-app/src/pages/chat/components/chat-message-list.tsx`
- `x-chat-provider`: custom Ant Design X provider work only when the current built-in `DeepSeekChatProvider` no longer fits
- `shadcn-ui`: generic primitive patterns through `slab-ui-primitives`
- `tauri-v2`: generic Tauri API details through `slab-tauri-app`
- `find-skills`: ecosystem discovery only; not a default route for repository work

## Architecture Overview

This workspace uses a supervisor-gateway plus gRPC worker pattern:

```text
slab-server  (HTTP gateway + supervisor, default :3000)
  |- API routes -> domain services -> gRPC gateway -> slab-runtime worker(s)
  `- SQLx persistence (SQLite)

slab-runtime  (standalone gRPC worker process)
  `- slab-core orchestrator + GGML backends (llama / whisper / diffusion)
```

- `slab-server` supervises child `slab-runtime` processes and proxies inference over gRPC or IPC. It must not embed model backends directly.
- `slab-runtime` is a self-contained gRPC server that runs enabled backends such as `llama`, `whisper`, and `diffusion`.
- `slab-core` is a runtime library used by `slab-runtime`; it must stay free of HTTP and SQL concerns.
- `slab-proto` owns the protobuf contract used between the supervisor and runtime.

### Crate Responsibilities

| Crate | Role |
|---|---|
| `slab-server` | HTTP/API gateway, supervisor, domain services, persistence |
| `slab-runtime` | gRPC server process, wraps `slab-core` |
| `slab-proto` | tonic/prost codegen from `proto/slab/ipc/v1/*.proto` |
| `slab-core` | Runtime facade, orchestrator, pipeline, engine backends |
| `slab-core-macros` | Procedural macros |
| `slab-llama` / `slab-whisper` / `slab-diffusion` | Safe FFI wrappers |
| `slab-llama-sys` / `slab-whisper-sys` / `slab-diffusion-sys` | Unsafe bindgen layers only |
| `slab-libfetch` | Model and binary download support |
| `slab-app` | Tauri desktop host plus React/Vite frontend |

## Code Organization

- Rust: keep module boundaries explicit. In `slab-server`, follow the existing `config`, `context`, `api`, `domain`, and `infra` layout instead of introducing parallel `state` or `routes` trees.
- Rust async: prefer Tokio-based async flows and the scheduler abstractions already in `slab-core/src/scheduler/`.
- Do not embed model backends directly in `slab-server`; inference should flow through `GrpcGateway -> slab-runtime -> slab-core`.
- Do not add HTTP or SQL dependencies to `slab-core` or `slab-runtime`.
- Do not move code across crates unless the responsibility boundary truly changes.

### Frontend Baseline

- `slab-app` is a React 19 + Vite + React Router 7 + Tauri 2 desktop app. It is not a single invoke-only shell.
- App bootstrap lives in `slab-app/src/App.tsx` and `slab-app/src/main.tsx`.
- Route composition lives in `slab-app/src/routes/index.tsx`.
- Server state uses TanStack Query v5 with `openapi-fetch` and `openapi-react-query`.
- Client state uses Zustand.
- AI-focused UI uses Ant Design X and `antd`.
- Chat surfaces currently use `DeepSeekChatProvider`, `useXChat`, `XRequest`, and `@ant-design/x-markdown` under `slab-app/src/pages/chat`.
- Shared primitives live under `slab-app/src/components/ui` and follow the existing Tailwind 4 + shadcn-style patterns.

## Key Files

- Server entry: `slab-server/src/main.rs`
- Server config: `slab-server/src/config.rs`
- HTTP router: `slab-server/src/api/mod.rs` and `slab-server/src/api/v1/mod.rs`
- Domain services: `slab-server/src/domain/services/`
- App state: `slab-server/src/context/mod.rs`
- gRPC gateway: `slab-server/src/infra/rpc/gateway.rs`
- Runtime entry: `slab-runtime/src/main.rs`
- Runtime gRPC: `slab-runtime/src/grpc/`
- Core facade: `slab-core/src/api/mod.rs`
- Protos: `slab-proto/proto/slab/ipc/v1/`
- Frontend app root: `slab-app/src/App.tsx`
- Frontend routes: `slab-app/src/routes/index.tsx`
- Frontend API client: `slab-app/src/lib/api/index.ts`
- Chat provider wiring: `slab-app/src/pages/chat/chat-context.ts`
- Chat hook: `slab-app/src/pages/chat/hooks/use-chat.ts`
- Tauri setup: `slab-app/src-tauri/src/setup.rs`
- Tauri config: `slab-app/src-tauri/tauri.conf.json`

## Build and Test

### Rust workspace

```sh
cargo build --workspace
cargo test --workspace
cargo check -p slab-server
cargo check -p slab-runtime
cargo run -p slab-server
```

### Frontend and Tauri

```sh
cd slab-app
bun install
bun run dev
bun run tauri dev
bun run build
bun run api
```

- `slab-proto` regenerates code through `tonic-build` during `cargo build`.
- `slab-app/src-tauri/tauri.conf.json` expects `bun run` for the frontend build hooks.

## Project Conventions

- API shape: `/v1/*` for product endpoints and `/health` for health checks.
- Long-running AI work should go through task entities and task-based flows rather than ad hoc status endpoints.
- Extend the existing `api/v1` modules instead of creating a parallel API tree.
- gRPC transport supports HTTP and IPC, controlled by `SLAB_TRANSPORT_MODE`.
- Model idle eviction is handled by `slab-server/src/model_auto_unload.rs`; do not bypass it for model lifecycle management.
- SQLx migrations live in `slab-server/migrations/` and should be append-only.
- The workspace excludes `slab-app/src` from Cargo operations; do not assume Rust tooling covers TypeScript sources.
- Prefer the built-in Ant Design X chat providers and current page-local chat wrappers before introducing a custom provider or a second chat state layer.

## Security and Runtime Notes

- Admin auth is optional unless `SLAB_ADMIN_TOKEN` is set.
- CORS defaults to allow-all unless `SLAB_CORS_ORIGINS` is configured.
- Swagger is enabled by default and can be disabled with `SLAB_ENABLE_SWAGGER=false`.
- Tauri CSP is explicitly configured in `slab-app/src-tauri/tauri.conf.json`; preserve or tighten it unless the task explicitly requires a change.
- Tauri capabilities currently include `main-window` and `plugin-webview`; do not broaden them casually.
- gRPC endpoints for `slab-runtime` should stay bound to localhost unless authenticated transport is added deliberately.

## AI Infrastructure Maintenance

- Keep `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, and relevant local skills in sync whenever the architecture or stack changes.
- Skill commands should be validated from the repo root on the shell they claim to support.
- Prefer local, versioned review references for repeatable audits; fetch remote guidance only when the user explicitly asks for the latest upstream rules.
