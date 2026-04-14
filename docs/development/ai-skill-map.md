# AI Skill Map

slab.rs no longer uses a separate project-skill wrapper layer. Most tasks should start from the codebase, and a local skill should only be opened when the task clearly matches it.

## Direct Skill Routing

- `use-x-chat`
  Use for `useXChat`, `useXConversations`, reload/abort flow, and page-local message state in `packages/slab-desktop/src/pages/chat/**`.
- `x-request`
  Use for chat transport, streaming params, and request wiring in `packages/slab-desktop/src/pages/chat/chat-context.ts`.
- `x-markdown`
  Use for assistant Markdown rendering and streaming content display in the chat UI.
- `x-chat-provider`
  Use only when the built-in `DeepSeekChatProvider` no longer matches the backend response shape.
- `shadcn-ui`
  Use for shared UI primitives, forms, dialogs, tables, and Tailwind-based component work in `packages/slab-components/`.
- `tauri-v2`
  Use for `bin/slab-app/src-tauri`, sidecar startup, capabilities, permissions, Tauri commands, plugin webview runtime, and desktop host integration details.

## No Local Skill Needed

- Agent control-plane work
  Start in `crates/slab-agent/**`, `slab-server/src/api/v1/agent/**`, and `slab-server/src/infra/agent_adapter.rs`.
- Runtime and engine work
  Start in `bin/slab-runtime/**`, especially `bin/slab-runtime/src/{config,context,domain,infra}/**` and `bin/slab-runtime/src/infra/backends/**`, plus `crates/slab-runtime-core/**`, `crates/slab-llama/**`, `crates/slab-whisper/**`, `crates/slab-diffusion/**`, and `crates/slab-ggml/**`.
- Plugin package and bridge work
  Start in `plugins/**`, `packages/slab-desktop/src/pages/plugins/**`, `packages/slab-desktop/src/lib/plugin-sdk.ts`, and `bin/slab-app/src-tauri/src/plugins/**`.
- Windows full-installer packaging work
  Start in `bin/slab-windows-full-installer/**`, `bin/slab-app/src-tauri/build.rs`, `bin/slab-app/src-tauri/installer-hooks/**`, and `Makefile.toml`. Add `tauri-v2` only when the change crosses into Tauri config or NSIS hook behavior.
- Shared contracts, settings, and manifests
  Start in `crates/slab-types/**`, `crates/slab-proto/**`, and `manifests/**`.

## Selection Order

- General repo work: read the code first, no skill required by default.
- Chat state behavior: `use-x-chat`
- Chat request wiring: `x-request`
- Chat Markdown rendering: `x-markdown`
- Custom chat provider adaptation: `x-chat-provider` only after confirming request config alone is not enough
- Shared frontend primitives or forms: `shadcn-ui`
- Tauri host, sidecar, capability, permission, or plugin webview work: `tauri-v2`
- Agent/runtime/plugin package changes outside those frontend/Tauri concerns: work directly from the code.

## Common Pairings

- Chat flow changes often need both `use-x-chat` and `x-request`.
- Chat rendering changes sometimes pair `use-x-chat` with `x-markdown`.
- A custom provider change should usually start with `use-x-chat` and `x-request`, then add `x-chat-provider` only if needed.
- Plugin UI work often starts in `packages/slab-desktop/src/pages/plugins/**` and adds `tauri-v2` only when the change crosses into `src-tauri`, capabilities, permissions, or plugin-view lifecycle code.
- Agent changes usually touch both `slab-server/src/api/v1/agent/**` and `crates/slab-agent/**` without any repo-local skill.
