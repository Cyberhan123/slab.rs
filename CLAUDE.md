# CLAUDE.md

Read [AGENTS.md](AGENTS.md) before making changes. This file only keeps the repo facts that are worth holding in short-term memory.

## Quick Rules

- `.agents/skills` contains optional task guidance. `plugins/` contains runtime plugin packages, not AI skills.
- Do not assume a project-specific skill wrapper layer exists. Use the codebase directly unless a task clearly matches one of the real local skills:
  - `use-x-chat`
  - `x-request`
  - `x-markdown`
  - `x-chat-provider`
  - `shadcn-ui`
  - `tauri-v2`
- Current frontend stack: React 19, Vite, React Router 7, Tauri 2, TanStack Query, `openapi-fetch`, `openapi-react-query`, Zustand, Ant Design X, `i18next`, and Tailwind 4.
- `bin/slab-app` is the Tauri desktop host. Its frontend is `packages/slab-desktop`, which uses `packages/slab-components` for UI and `packages/slab-i18n` for i18n. The desktop host starts `bin/slab-server` as a local sidecar, keeps product API traffic on HTTP, and reserves Tauri commands for host-only features such as plugin runtime integration.
- `packages/slab-components` is the shadcn/ui-based shared component library (workspace package `@slab/components`).
- `packages/slab-i18n` is the shared i18n package (workspace package `@slab/i18n`) with i18next and react-i18next.
- `packages/slab-plugin-ui` is the stable plugin UI ABI package (`@slab/plugin-ui`) and exports only the safe plugin component subset plus plugin-scoped global styles.
- `packages/slab-plugin-sdk` is the plugin-author SDK package (workspace package `@slab/plugin-sdk`) that wraps the webview host bridge, theme snapshots, JSON API helpers, and plugin integrity generation for plugin pages.
- `packages/slab-desktop` is the main React frontend app (workspace package `@slab/desktop`).
- `packages/vitest-rust-reporter` is the workspace helper that maps Rust `cargo test` and optional `cargo llvm-cov` results into a Vitest project for `vitest --ui`.
- Frontend/workspace lint now runs from the repo root with `bun run lint`, and auto-fixes use `bun run lint:fix`.
- Public VitePress pages live in `docs/`, internal contributor docs live in `docs/development/`, and published JSON Schemas are generated into `docs/public/manifests/v1/` with `bun run docs:schemas`.
- All Rust library crates live in `crates/` (e.g., `crates/slab-runtime-core`, `crates/slab-types`, `crates/slab-agent`, `crates/slab-agent-tools`, `crates/slab-app-core`).
- Binary executables live in `bin/` (e.g., `bin/slab-server`, `bin/slab-runtime`, `bin/slab-app`).
- `crates/slab-app-core` (package: `slab-app-core`) is the HTTP-free business logic library behind `bin/slab-server`. Contains `context/`, `domain/`, `infra/`, `config`, `model_auto_unload`, and the shared `runtime_supervisor`. Migrations are in `crates/slab-app-core/migrations/`.
- `crates/slab-hub` is the feature-gated model hub abstraction for `slab-app-core`; it centralizes repo file listing, downloads, reachability probing, and provider fallback across supported hub crates.
- `bin/slab-server` is the thin HTTP gateway (axum) and headless host. It depends on `crates/slab-app-core` for all domain logic; adds axum `FromRef` extractors in `state_extractors.rs`, `ServerError` → HTTP response conversion, and uses the shared core runtime supervisor through a `tokio::process` adapter. Exposes `/v1` plus `/api-docs/openapi.json`.
- `bin/slab-runtime` serves gRPC over TCP or IPC and is the only runtime composition root. It now uses an in-package `bootstrap` / `api` / `application` / `domain` / `infra` layout, with system startup in `bin/slab-runtime/src/bootstrap`, gRPC handlers in `bin/slab-runtime/src/api/handlers`, application orchestration in `bin/slab-runtime/src/application/services`, and flattened GGML, Candle, and ONNX backend implementations under `bin/slab-runtime/src/infra/backends`.
- `bin/slab-windows-full-installer` is the Windows-only outer bootstrap that packages the resource-less Tauri NSIS installer with CAB runtime payloads and rehydrates `resources/libs` during install via NSIS hooks.
- `crates/slab-runtime-core` (package: `slab-runtime-core`) is the pure scheduler/backend-protocol crate; backend composition and typed runtime codecs belong in `bin/slab-runtime`, and shared contracts belong in `crates/slab-types` and `crates/slab-proto`.
- `crates/slab-agent` is the pure agent control-plane crate. Concrete host tools belong in `crates/slab-agent-tools`; plugin/API capability adapters are registered by host/app-core layers.
- Plugin manifests now support `manifestVersion: 1` with runtime assets, host-controlled `contributes.*`, `permissions.*`, and agent capabilities. `plugin.json` remains the static source of truth, while `/v1/plugins/*` plus the plugin state table track dynamic install/runtime state. MCP is a future export target for capabilities, not the plugin runtime itself.
- The default third-party plugin UI model is a sandboxed Tauri child WebView with token mirroring through `@slab/plugin-sdk`; do not make Module Federation the default plugin runtime. Use `bun run build:plugins` from the repo root to build local plugin frontends and refresh manifest integrity.
- Preserve the current Tauri CSP, permissions, capabilities, and plugin host boundaries unless the task explicitly requires a change.
- If repo docs and code disagree, follow the code and update the docs.
