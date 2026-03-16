---
name: use-x-chat
description: Use when working with useXChat or useXConversations from @ant-design/x-sdk, especially in slab-app/src/pages/chat. Covers message state, request lifecycles, reload and abort behavior, placeholder and fallback messages, and conversation wiring in this repository.
---

# use-x-chat

Use this skill when a task directly touches the chat state layer in `slab-app/src/pages/chat/**`.

## Repo Defaults

- The current chat flow is built around `slab-app/src/pages/chat/chat-context.ts`.
- `providerFactory` currently returns a built-in `DeepSeekChatProvider`.
- `slab-app/src/pages/chat/hooks/use-chat.ts` wraps `useXChat` and owns submit, reload, and abort behavior.
- Placeholder and fallback assistant messages are already handled in the local wrapper hook.
- Conversation-local state should stay close to the chat page unless reuse across routes is proven.

## Workflow

1. Read `slab-app/src/pages/chat/chat-context.ts` and `slab-app/src/pages/chat/hooks/use-chat.ts` before changing chat state behavior.
2. Preserve the current wrapper pattern unless the user explicitly asks for a different architecture.
3. Keep `requestPlaceholder`, `requestFallback`, `onReload`, and `beforeRequest` behavior coherent when changing request flow.
4. If transport, params, or auth-safe request wiring changes, also open `../x-request/SKILL.md`.
5. If the backend shape no longer matches the built-in provider, also open `../x-chat-provider/SKILL.md`.
6. If the task changes assistant Markdown rendering, also open `../x-markdown/SKILL.md`.

## Read On Demand

- `reference/CORE.md` for the hook surface area and common message operations
- `reference/API.md` for typed signatures
- `reference/EXAMPLES.md` for example component patterns

## Avoid

- Do not instantiate a new provider on every render.
- Do not duplicate `useXChat` message state in Zustand or another store unless there is clear cross-route reuse.
- Do not remove abort, reload, or fallback behavior without replacing it intentionally.
- Do not put secrets or third-party API keys in the frontend chat layer.

## Done When

- Chat requests, reloads, and aborts still flow through the current page-local wrapper.
- Conversation state remains understandable from the chat page entry points.
- Any related transport or rendering changes stay aligned with the matching x-skills.
