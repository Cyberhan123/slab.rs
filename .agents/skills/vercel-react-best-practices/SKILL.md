---
name: vercel-react-best-practices
description: React performance guidance from Vercel. Use for React reviews or refactors, but only apply Next.js or RSC rules when the target code actually uses those technologies.
license: MIT
metadata:
  author: vercel
  version: "1.0.0"
---

# Vercel React Best Practices

Use this skill to review or refactor React code with a performance mindset.

## Repo-Specific Scope

- `slab-app` uses React 19 + Vite + React Router 7 + Tauri 2. It is not a Next.js app.
- The existing data layer is TanStack Query with `openapi-fetch` and `openapi-react-query`, not SWR.
- For this repo, treat Next.js, RSC, and Server Action rules as conditional guidance, not defaults.

## Prioritize These Rule Families In This Repo

- Async sequencing: `async-defer-await`, `async-parallel`, `async-dependencies`
- Client behavior: `client-event-listeners`, `client-passive-event-listeners`, `client-localstorage-schema`
- Re-render control: `rerender-derived-state-no-effect`, `rerender-dependencies`, `rerender-move-effect-to-event`, `rerender-transitions`, `rerender-use-ref-transient-values`
- Rendering: `rendering-content-visibility`, `rendering-conditional-render`, `rendering-hoist-jsx`
- General JavaScript hot paths: the `js-*` rules in `rules/`

## Translate, Do Not Blindly Port

- If a rule mentions SWR deduplication, apply the same idea using the existing React Query layer.
- If a rule mentions `next/dynamic`, Server Actions, RSC boundaries, or `React.cache()`, skip it unless you are working in an actual Next.js codebase.
- Do not introduce Next.js-only APIs into `slab-app`.

## How To Use The Skill

1. Start from the task at hand: bundle size, re-renders, event listeners, long lists, or async waterfalls.
2. Open only the relevant files under `rules/`.
3. Apply repo-compatible rules first.
4. Call out any skipped Next.js-specific guidance explicitly when it would otherwise be tempting to apply.

## Good Starting Rules

- `rules/async-parallel.md`
- `rules/rerender-derived-state-no-effect.md`
- `rules/rerender-move-effect-to-event.md`
- `rules/rerender-transitions.md`
- `rules/client-event-listeners.md`
- `rules/rendering-content-visibility.md`

## Skip Unless The Target Really Is Next.js

- `rules/bundle-dynamic-imports.md`
- `rules/server-auth-actions.md`
- `rules/server-cache-react.md`
- `rules/server-dedup-props.md`
- `rules/server-serialization.md`
- `rules/server-after-nonblocking.md`

For broader repo context, read `AGENTS.md` in the project root.
