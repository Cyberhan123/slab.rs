---
name: x-chat-provider
description: Use when adapting a nonstandard streaming chat API to an Ant Design X provider, especially if replacing the current built-in DeepSeekChatProvider in slab-app/src/pages/chat/chat-context.ts. Covers custom provider classes, parameter mapping, stream normalization, and when not to build a custom provider in this repository.
---

# x-chat-provider

Use this skill only when the built-in Ant Design X providers no longer fit the backend contract.

## Repo Defaults

- The current chat page uses a built-in `DeepSeekChatProvider` in `slab-app/src/pages/chat/chat-context.ts`.
- A custom provider is not the default path in this repository.
- `XRequest` should continue to own network transport even when a custom provider is required.
- Most chat tasks in this repo should start with `../use-x-chat/SKILL.md` and `../x-request/SKILL.md` instead.

## When To Reach For This Skill

- The backend response shape cannot be consumed by the current built-in provider.
- Request params or stream events need custom mapping that request config alone cannot express.
- The task explicitly asks for a new provider abstraction.

## Workflow

1. Inspect `slab-app/src/pages/chat/chat-context.ts` and confirm that request config alone is not enough.
2. Keep the provider close to the existing chat page or chat library code so the integration remains easy to trace.
3. Let `XRequest` handle transport and focus the provider on request and response adaptation.
4. Map request params from the existing page-local chat flow instead of inventing a second input contract.
5. Recheck `useXChat` behavior after any provider change by also opening `../use-x-chat/SKILL.md`.

## Read On Demand

- `reference/EXAMPLES.md` for provider examples

## Avoid

- Do not create a custom provider just to rename a small set of fields.
- Do not move transport logic out of `XRequest` unless there is a strong reason.
- Do not leak backend-specific provider types across unrelated page components.
- Do not replace the built-in provider without checking whether the backend can be normalized first.

## Done When

- The reason for a custom provider is explicit and real.
- Request transport, provider mapping, and page-level chat state still have clear ownership boundaries.
- The new provider integrates cleanly with the existing chat page entry points.
