# Project Guidelines

## Agent Workflow

- Read this file before making changes.
- Trust the current code when docs and implementation disagree, then update the docs.
- Keep repo guidance deterministic: commands should work from the repo root, and local references should come before remote-only advice.
- `.agents/skills` contains optional task guidance, not generic app source.
- The project is stable now, so do not force a skill-selection step for every task. Read the code first and only open a skill when the task directly matches it.

## Local Skills

Only the skills that actually exist in `.agents/skills` should be treated as repo-local defaults:

- `use-x-chat`: chat state and `useXChat` / `useXConversations` work in `slab-app/src/pages/chat/**`
- `x-request`: frontend chat transport changes in `slab-app/src/pages/chat/chat-context.ts`
- `x-markdown`: assistant Markdown rendering in `slab-app/src/pages/chat/components/chat-message-list.tsx`
- `x-chat-provider`: custom Ant Design X provider work, only when the built-in `DeepSeekChatProvider` no longer fits the backend contract
- `shadcn-ui`: shared React UI primitives, forms, and Tailwind-based component patterns
- `tauri-v2`: `slab-app/src-tauri`, capabilities, commands, IPC, and Tauri runtime details

If a task does not clearly match one of the skills above, work directly from the codebase without adding an extra routing layer.

See [docs/ai-skill-map.md](docs/ai-skill-map.md) for the short routing map.

## Architecture Snapshot

This workspace uses a supervisor plus runtime split:

```text
slab-server  ->  slab-runtime  ->  slab-core
   HTTP          gRPC worker      runtime/orchestrator
```

- `slab-server` is the HTTP gateway, supervisor, and persistence layer.
- `slab-runtime` is the standalone worker process.
- `slab-core` holds runtime orchestration and backend-facing logic.
- `slab-proto` owns the protobuf contract between server and runtime.
- `slab-types` is the shared semantic types and JSON schema crate used across the Rust workspace.
- `slab-app` is the Tauri desktop host with the React frontend.

## Repo Layout

- `slab-server`: keep the existing `config`, `context`, `api`, `domain`, and `infra` layout.
- `slab-runtime`: gRPC service and runtime process entry points.
- `slab-core`: runtime library only; keep HTTP and SQL concerns out.
- `slab-types`: workspace-wide shared semantic types, schema definitions, and other reusable Rust contracts.
- `slab-app/src`: React 19 + Vite + React Router 7 frontend.
- `slab-app/src/pages/chat`: Ant Design X chat UI and page-local wrappers.
- `slab-app/src/components/ui`: shared UI primitives.
- `slab-app/src-tauri`: Tauri host, commands, setup, capabilities, and security boundaries.
- `slab-proto/proto/slab/ipc/v1`: supervisor/runtime protobuf definitions.

## Working Rules

- Keep inference behind `slab-server -> GrpcGateway -> slab-runtime -> slab-core`.
- Extend the existing `/v1/*` API modules instead of adding a parallel API tree.
- Keep long-running AI work in task-oriented flows when the feature already follows that model.
- Prefer `slab-types` for Rust types that need to be shared cleanly across crates instead of duplicating contracts.
- SQLx migrations in `slab-server/migrations/` are append-only.
- Preserve Tauri CSP and capability boundaries unless the task explicitly requires a change.
- Prefer the existing page-local chat wrappers and built-in providers before introducing a new chat abstraction.
- Cargo excludes `slab-app/src`, so Rust tooling does not validate the TypeScript frontend.

## Build and Test

From the repo root:

```sh
cargo build --workspace
cargo test --workspace
cargo check -p slab-server
cargo check -p slab-runtime
```

Frontend and Tauri:

```sh
cd slab-app
bun install
bun run dev
bun run build
bun run tauri dev
bun run api
```

## AI Docs Maintenance

- Keep `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, and `docs/ai-skill-map.md` aligned when the skill list or workflow changes.
- Do not document repo-local skills that do not exist on disk.
