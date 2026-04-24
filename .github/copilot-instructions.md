Read [../AGENTS.md](../AGENTS.md) before making changes.

Key repo facts:

- `.agents/skills` contains optional operational skills. `plugins/` is the desktop runtime plugin workspace and should not be treated as skill content.
- Do not assume a local slab.rs skill wrapper layer exists. Use the codebase directly unless the task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- `bin/slab-app` is the Tauri 2 desktop host that mounts local plugin webviews, starts `bin/slab-server` as a local sidecar, and keeps product API traffic on HTTP. Tauri commands are reserved for host-only features such as plugin runtime integration.
- `packages/slab-desktop` is the React 19 + Vite + React Router 7 frontend app, managed as a bun workspace package (`@slab/desktop`).
- `packages/slab-components` is the shared shadcn/ui-based React component library (`@slab/components`), with Radix UI + Tailwind 4 primitives.
- `packages/slab-i18n` is the shared i18n package (`@slab/i18n`) using i18next and react-i18next.
- `packages/slab-plugin-ui` is the stable plugin UI ABI package (`@slab/plugin-ui`) for the safe plugin component subset and plugin-scoped global styles.
- `packages/slab-plugin-sdk` is the plugin-author SDK package (`@slab/plugin-sdk`) for stable plugin webview host calls, theme snapshots, JSON API helpers, and manifest integrity generation.
- `packages/vitest-rust-reporter` is the workspace helper package that projects Rust `cargo test` and optional `cargo llvm-cov` results into the Vitest UI.
- Frontend/workspace lint runs from the repo root with `bun run lint`, and auto-fixes use `bun run lint:fix`.
- Public VitePress pages live in `docs/`, internal contributor docs live in `docs/development/`, and published JSON Schemas are generated into `docs/public/manifests/v1/` with `bun run docs:schemas`.
- Frontend server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Frontend client state uses Zustand.
- AI-focused frontend components use Ant Design X, with shared Tailwind 4 primitives from `packages/slab-components/src`.
- `crates/slab-app-core` (directory: `crates/slab-app-core/`) holds the HTTP-free business logic used by `bin/slab-server`: `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`, and `runtime_supervisor`. Migrations live in `crates/slab-app-core/migrations/`.
- `crates/slab-hub` is the feature-gated hub abstraction for `slab-app-core`, providing unified repo listing/download APIs plus reachability-based provider fallback across supported hub crates.
- `bin/slab-server` is the thin HTTP (axum) gateway and headless host. It depends on `crates/slab-app-core` for domain/infra logic; `state_extractors.rs` provides axum `FromRef` impls, `error.rs` provides `ServerError` → HTTP response conversion, and runtime process supervision is delegated to the shared core supervisor through a `tokio::process` adapter. The server exposes `/v1` plus `/api-docs/openapi.json`.
- AI inference must stay behind `host -> bin/slab-server -> crates/slab-app-core runtime supervisor -> GrpcGateway -> bin/slab-runtime local composition layer -> crates/slab-runtime-core`.
- `bin/slab-runtime` supports TCP or IPC transport and is the only runtime composition root. It uses an in-package `bootstrap` / `api` / `application` / `domain` / `infra` layout, with system startup in `bin/slab-runtime/src/bootstrap`, gRPC handlers in `bin/slab-runtime/src/api/handlers`, application orchestration in `bin/slab-runtime/src/application/services`, and flattened GGML, Candle, and ONNX backends under `bin/slab-runtime/src/infra/backends`.
- `bin/slab-windows-full-installer` is the Windows-only full-installer bootstrap. It packs the resource-less Tauri NSIS installer with CAB payloads, expands runtime files into `%TEMP%`, and uses NSIS hooks plus its `apply` helper mode to hydrate `$INSTDIR/resources/libs`.
- `crates/slab-runtime-core` (package: `slab-runtime-core`) holds the scheduler, backend protocol, worker runner, task state, and generic payload/error types. Keep HTTP, SQL, typed runtime codecs, and backend composition concerns out.
- `crates/slab-types` and `crates/slab-proto` are the shared Rust contract crates for semantic types, settings/runtime models, and server/runtime IPC.
- All Rust library crates live in `crates/` (e.g., `crates/slab-runtime-core`, `crates/slab-types`, `crates/slab-agent`, `crates/slab-agent-tools`, `crates/slab-app-core`).
- Binary executables live in `bin/` (e.g., `bin/slab-server`, `bin/slab-runtime`, `bin/slab-app`).
- `crates/slab-agent` is the pure agent control-plane crate. Concrete host tools belong in `crates/slab-agent-tools`; plugin/API capability adapters are registered by host/app-core layers.
- Plugin manifests support `manifestVersion: 1` with runtime assets, host-controlled `contributes.*`, `permissions.*`, and agent capabilities. `plugin.json` stays the static source of truth; `/v1/plugins/*` and the plugin state table track dynamic install/runtime state. MCP is a capability export target, not the plugin runtime itself.
- The default third-party plugin UI model is a sandboxed Tauri child WebView, not Module Federation. Use `@slab/plugin-ui` and `@slab/plugin-sdk` for plugin UIs, and run `bun run build:plugins` from the repo root to build local plugin assets, refresh integrity hashes, and emit `.plugin.slab` packs.
- Tauri security settings are explicit in `bin/slab-app/src-tauri/tauri.conf.json`; preserve CSP, permissions, capabilities, and plugin boundaries unless the task requires a deliberate change.
- If documentation and code disagree, trust the code and update the documentation.
