# AI Skill Map

This file maps slab.rs tasks to the local skills under `.agents/skills`.

## Project Skills

- `slab-frontend-page`
  Use for routed page work in `slab-app/src/pages`, `routes`, `layouts`, and page-local chat surfaces.
- `slab-ui-primitives`
  Use for shared UI primitives, form building blocks, and theme-level frontend patterns.
- `slab-tauri-app`
  Use for `src-tauri`, capabilities, sidecars, plugin webviews, and desktop security boundaries.
- `slab-server-feature`
  Use for `slab-server` API, service, context, persistence, and gateway-side feature work.
- `slab-runtime-async`
  Use for scheduler and async runtime internals in `slab-core` and `slab-runtime`.
- `slab-ui-review`
  Use for project-specific UI review and pre-merge UX/accessibility checks.

## Support Skills To Reference Through Project Skills

- `frontend-design`
  Use through `slab-frontend-page` when the task needs a stronger visual direction.
- `ui-ux-pro-max`
  Use through `slab-frontend-page` or `slab-ui-review` when you want a design-system-first approach or remediation ideas.
- `use-x-chat`
  Use through `slab-frontend-page` for `useXChat` and `useXConversations` work in `slab-app/src/pages/chat/**`.
- `x-request`
  Use through `slab-frontend-page` for `XRequest` transport, streaming, and auth-safe frontend request wiring.
- `x-markdown`
  Use through `slab-frontend-page` or `slab-ui-review` for assistant Markdown rendering and streaming content behavior.
- `x-chat-provider`
  Use through `slab-frontend-page` only when the built-in `DeepSeekChatProvider` no longer matches the backend shape.
- `shadcn-ui`
  Use through `slab-ui-primitives` for generic primitive patterns.
- `tauri-v2`
  Use through `slab-tauri-app` for Tauri API details and common pitfalls.

## Selection Order

- Page feature in `slab-app`: `slab-frontend-page`
- Chat UI in `slab-app/src/pages/chat`: `slab-frontend-page`, then `use-x-chat`, `x-request`, or `x-markdown` as needed; add `x-chat-provider` only for custom provider work
- Shared component or theme work: `slab-ui-primitives`
- Tauri host or permission work: `slab-tauri-app`
- HTTP gateway feature in `slab-server`: `slab-server-feature`
- Runtime orchestration or backend concurrency: `slab-runtime-async`
- UI review: `slab-ui-review`, with `ui-ux-pro-max` only when you need design remediation ideas
