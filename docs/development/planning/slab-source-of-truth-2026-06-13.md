# Slab Source-of-Truth Checklist

> Date: 2026-06-13  
> Scope: phase 0 baseline calibration for the Slab enhancement roadmap.  
> Verified with: `.\target\debug\slab-server.exe --print-openapi`, root `package.json`, root `Cargo.toml`, and current route/schema source files.

This checklist is the first stop before implementing roadmap phases 1-6. The roadmap's long-term opportunity pool is not part of the execution scope.

## Root Workflow

- Root scripts: `package.json`.
- Rust workspace members: `Cargo.toml`.
- Day-to-day validation stays behind Bun + Cargo root commands.
- Do not reintroduce `cargo make`, Bazel wrappers, or a second top-level build orchestrator.

## API Routes

Current route sources:

- Router composition: `bin/slab-server/src/api/v1/mod.rs`.
- Route handlers: `bin/slab-server/src/api/v1/*/handler.rs`.
- Generated TypeScript surface: `packages/api/src/v1.d.ts`.
- Smoke route inventory: `bin/slab-server/tests/smoke/server-api/shared.ts`.
- Live OpenAPI: `.\target\debug\slab-server.exe --print-openapi` or `/api-docs/openapi.json`.

Confirmed live route groups:

- Health: `GET /health`.
- Agent: `GET|POST /v1/agents/responses`.
- Chat: `POST /v1/chat/completions`, `POST /v1/completions`, `GET /v1/chat/models`.
- Backends: `GET /v1/backends`, `GET /v1/backends/status`.
- Models: `GET|POST /v1/models`, `GET|PUT|DELETE /v1/models/{id}`, `GET /v1/models/available`, `POST /v1/models/import-pack`, `POST /v1/models/download`, `POST /v1/models/load`, `POST /v1/models/unload`, `POST /v1/models/switch`.
- Tasks: `GET /v1/tasks`, `GET /v1/tasks/{id}`, `GET /v1/tasks/{id}/result`, `POST /v1/tasks/{id}/cancel`, `POST /v1/tasks/{id}/restart`.
- Media: `GET|POST /v1/audio/transcriptions`, `GET /v1/audio/transcriptions/{id}`, `GET|POST /v1/images/generations`, `GET /v1/images/generations/{id}`, `GET /v1/images/generations/{id}/artifacts/{index}`, `GET /v1/images/generations/{id}/reference`, `GET|POST /v1/video/generations`, `GET /v1/video/generations/{id}`, `GET /v1/video/generations/{id}/artifact`, `GET /v1/video/generations/{id}/reference`, `POST /v1/ffmpeg/convert`, `POST /v1/subtitles/render`.
- Plugins: `GET /v1/plugins`, `POST /v1/plugins/install`, `POST /v1/plugins/import-pack`, `GET /v1/plugins/rpc`, `GET /v1/plugins/events`, `GET|DELETE /v1/plugins/{id}`, `POST /v1/plugins/{id}/enable`, `POST /v1/plugins/{id}/disable`, `POST /v1/plugins/{id}/start`, `POST /v1/plugins/{id}/stop`.
- Sessions: `GET|POST /v1/sessions`, `DELETE|PUT /v1/sessions/{id}`, `GET /v1/sessions/{id}/messages`.
- Settings and setup: `GET /v1/settings`, `GET|PUT /v1/settings/{pmid}`, `GET /v1/setup/status`, `POST /v1/setup/provision`, `POST /v1/setup/complete`.
- Workspace and UI state: `GET /v1/workspace/lsp/{language}`, `GET|PUT|DELETE /v1/ui-state/{key}`.
- System: `GET /v1/system/gpu`.

Do not document `GET /v1/chat/completions`, `/v1/image/*`, `/api/tasks/*`, `DELETE /v1/tasks/{id}`, or `/admin/*` as current routes unless source code reintroduces them.

## Settings Schema

Current sources:

- Settings document: `crates/slab-config/src/settings/document.rs`.
- PMIDs and section metadata: `crates/slab-config/src/settings/pmid.rs` and `crates/slab-config/src/pmid_service.rs`.
- Descriptors: `crates/slab-config/src/descriptor.rs`.
- Generated schema: `docs/public/manifests/v1/settings-document.schema.json`.

Validation:

- Run `bun run gen:schemas` after settings schema changes.
- Agent memory, hook, MCP, and websearch settings are current under `agent.memories.*`, `agent.hooks.*`, `agent.tools.mcp.*`, and `agent.tools.websearch.*`.
- MCP stdio server launch configs are stored at `agent.tools.mcp.servers` and are exposed through the Settings PMID view with env values represented as host environment variable references.

## Plugin Manifest

Current sources:

- Runtime validation: `crates/slab-plugin/src/registry.rs`.
- Business validation: `crates/slab-app-core/src/domain/services/plugin/validation.rs`.
- Pack validation: `packages/slab-plugin-cli/src/pack.ts`.
- Generated manifest schema: `docs/public/manifests/v1/slab-manifest.schema.json`.
- Plugin workspace guidance: `plugins/README.md`.

Validation:

- Run `bun run gen:plugin-packs` after plugin pack or bundled plugin changes.
- Keep `manifestVersion: 1`, `runtime.ui.entry`, `permissions.*`, and `contributes.*` as the static plugin contract.
- Plugin WebView caller identity remains derived from the WebView label, not plugin payload fields.

## Agent Events And State

Current sources:

- Shared status enums: `crates/slab-types/src/agent.rs`.
- Agent events: `crates/slab-agent/src/event.rs`.
- Agent control/state transitions: `crates/slab-agent/src/control.rs` and `crates/slab-agent/src/state.rs`.
- App-core agent event hub: `crates/slab-app-core/src/infra/agent_event_hub.rs`.
- Agent HTTP/WebSocket route schemas: `crates/slab-app-core/src/schemas/agent.rs`.

Confirmed status values:

- Thread: `pending`, `running`, `interrupting`, `interrupted`, `completed`, `errored`, `shutdown`.
- Tool call: `pending`, `running`, `completed`, `failed`.

Validation:

- Use `cargo check -p slab-agent -p slab-agent-tools -p slab-agent-memories -p slab-app-core` for agent/tool/memory boundary changes.
- API shape changes require `bun run gen:api`.

## Runtime Protocol

Current sources:

- Runtime composition root: `bin/slab-runtime`.
- Runtime supervisor and gRPC gateway integration: `crates/slab-app-core/src/infra/runtime/` and `crates/slab-app-core/src/infra/rpc/gateway.rs`.
- Runtime protocol substrate: `crates/slab-runtime-core`.
- Protobuf contracts: `crates/slab-proto/proto/slab/ipc/v1/`.

Confirmed runtime services:

- `GgmlLlamaService`, `GgmlWhisperService`, `GgmlDiffusionService`.
- `CandleTransformersService`, `CandleDiffusionService`.
- `OnnxService`.

Validation:

- Runtime API or protobuf changes require the narrowest matching Cargo check, usually `cargo check -p slab-runtime -p slab-runtime-core -p slab-app-core`.
- Keep `bin/slab-runtime` as the runtime composition root; do not move business logic into `crates/slab-runtime-core`.

## Frontend Entry Points

Current sources:

- Desktop app routes and pages: `packages/slab-desktop/src/pages/`.
- OpenAPI client types: `packages/api/src/v1.d.ts`.
- UI state storage: `packages/slab-desktop/src/store/ui-state-storage.ts`.
- Workspace LSP client: `packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts`.

Validation:

- Frontend route/API changes require `bun run check:frontend`.
- Backend API shape changes require `bun run gen:api` before frontend validation.
