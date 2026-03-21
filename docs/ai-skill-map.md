# AI Skill Map

slab.rs no longer uses a separate project-skill wrapper layer. Most tasks should start from the codebase, and a local skill should only be opened when the task clearly matches it.

## Direct Skill Routing

- `use-x-chat`
  Use for `useXChat`, `useXConversations`, reload/abort flow, and page-local message state in `slab-app/src/pages/chat/**`.
- `x-request`
  Use for chat transport, streaming params, and request wiring in `slab-app/src/pages/chat/chat-context.ts`.
- `x-markdown`
  Use for assistant Markdown rendering and streaming content display in the chat UI.
- `x-chat-provider`
  Use only when the built-in `DeepSeekChatProvider` no longer matches the backend response shape.
- `shadcn-ui`
  Use for shared UI primitives, forms, dialogs, tables, and Tailwind-based component work.
- `tauri-v2`
  Use for `src-tauri`, Tauri commands, capabilities, IPC, plugins, and runtime integration.

## Selection Order

- General repo work: read the code first, no skill required by default.
- Chat state behavior: `use-x-chat`
- Chat request wiring: `x-request`
- Chat Markdown rendering: `x-markdown`
- Custom chat provider adaptation: `x-chat-provider` only after confirming request config alone is not enough
- Shared frontend primitives or forms: `shadcn-ui`
- Tauri host and permission work: `tauri-v2`

## Common Pairings

- Chat flow changes often need both `use-x-chat` and `x-request`.
- Chat rendering changes sometimes pair `use-x-chat` with `x-markdown`.
- A custom provider change should usually start with `use-x-chat` and `x-request`, then add `x-chat-provider` only if needed.
