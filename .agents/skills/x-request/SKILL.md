---
name: x-request
description: Use when configuring XRequest from @ant-design/x-sdk, especially in slab-app/src/pages/chat/chat-context.ts. Covers manual mode, streaming request params, abort behavior, proxy-safe auth, and frontend transport wiring for Ant Design X chat flows in this repository.
---

# x-request

Use this skill when a task changes how the frontend chat layer talks to the backend.

## Repo Defaults

- The current request wiring lives in `slab-app/src/pages/chat/chat-context.ts`.
- `XRequest` is pointed at `${API_BASE_URL}/v1/chat/completions`.
- The current provider uses `manual: true` and streaming params such as `stream`, `model`, and `id`.
- `API_BASE_URL` is derived from `VITE_API_BASE_URL` and falls back to `http://localhost:3000`.
- The frontend should not carry provider secrets or direct third-party model credentials.

## Workflow

1. Start from the existing `XRequest` call in `slab-app/src/pages/chat/chat-context.ts`.
2. Preserve `manual: true` for provider-driven request lifecycles unless you are deliberately changing the architecture.
3. Keep request params, abort semantics, and streaming behavior aligned with `useXChat`.
4. If the API shape changes but still fits a built-in provider, update request config first before inventing a custom provider.
5. If the built-in provider can no longer consume the backend response shape, also open `../x-chat-provider/SKILL.md`.
6. If the task changes chat state behavior, also open `../use-x-chat/SKILL.md`.

## Read On Demand

- `reference/CORE.md` for common `XRequest` options and lifecycle notes
- `reference/API.md` for the request API surface
- `reference/EXAMPLES_SERVICE_PROVIDER.md` for service-specific examples

## Avoid

- Do not hardcode credentials or Authorization headers for third-party providers in the frontend.
- Do not remove `manual: true` from a provider-backed request unless you also redesign the call flow.
- Do not scatter chat request config across multiple components when `chat-context.ts` already centralizes it.
- Do not bypass the local server boundary casually if the desktop app already proxies the request safely.

## Done When

- Request configuration still has a single obvious home.
- Streaming, abort, and model-selection behavior remain compatible with the page-local chat hook.
- Security-sensitive auth stays on the safe side of the frontend boundary.
