---
name: x-markdown
description: Use when rendering assistant Markdown with @ant-design/x-markdown, especially in slab-app/src/pages/chat/components/chat-message-list.tsx. Covers streaming Markdown, theme classes, custom renderers, and chat-oriented rich content behavior in this repository.
---

# x-markdown

Use this skill when a task changes how assistant content is rendered after chat data has already been produced.

## Repo Defaults

- Markdown rendering currently lives in `slab-app/src/pages/chat/components/chat-message-list.tsx`.
- Theme selection is handled by `slab-app/src/pages/chat/hooks/use-markdowm-theme.ts`.
- The current page imports the built-in light and dark theme CSS from `@ant-design/x-markdown`.
- Streaming assistant output should remain readable even while the message is incomplete.

## Workflow

1. Read `slab-app/src/pages/chat/components/chat-message-list.tsx` and `slab-app/src/pages/chat/hooks/use-markdowm-theme.ts` first.
2. Keep the chosen theme class aligned with the current light and dark mode flow.
3. Add custom component mapping only when the content truly needs a richer renderer.
4. If the task changes how assistant messages are produced, also open `../use-x-chat/SKILL.md`.
5. If the task changes transport or stream chunk behavior, also open `../x-request/SKILL.md`.

## Read On Demand

- `reference/CORE.md` for base rendering patterns
- `reference/STREAMING.md` for incomplete or streaming Markdown behavior
- `reference/EXTENSIONS.md` for plugins and richer renderers
- `reference/API.md` for the component API

## Avoid

- Do not parse the same Markdown content twice.
- Do not introduce unsafe raw HTML handling unless the user explicitly asks for it and the safety tradeoff is understood.
- Do not drift away from the existing light and dark theme classes without updating the matching hook.
- Do not move chat-specific Markdown decisions into shared primitives unless reuse is already established.

## Done When

- Assistant content still renders correctly for both complete and streaming messages.
- Theme behavior stays aligned with the current chat page.
- Any richer rendering stays scoped to the message surface that needs it.
