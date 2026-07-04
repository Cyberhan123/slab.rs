---
name: front-end-code-style
description: Use when building or refactoring React Router + Vite front-end code in this repo, including TypeScript, React components, hooks, data fetching through the generated api.useQuery/api.useMutation/api.useXXX hooks, shadcn UI composition, bundle performance, render performance, and lint-consistent code style.
---

# Front-end Code Style

Apply this skill for React Router + Vite application code. Treat the repo's generated `api` package, shadcn components, and local project organization rules as the default architecture.

## Repo-specific rules

- Use `api.useQuery`, `api.useMutation`, or other generated `api.useXXX` hooks for slab-server API calls. Do not call `fetch`, axios, or raw TanStack Query hooks for generated API routes unless you are implementing the API package itself.
- Regenerate contracts with `bun run gen:api` and schemas with `bun run gen:schemas`; never patch generated API/schema output by hand.
- Prefer existing hooks, utilities, packages, and shared components before adding custom implementations.
- Inline helper functions with fewer than 5 lines of logic when they are called only once, unless a thin adapter is required by an external hook or package API.
- After front-end edits, run the narrowest useful validation first. For lintable repo code, run `bun run lint:fix`; for wider front-end changes, use `bun run check:frontend` when appropriate.

## Architecture fit

- Use `react-router` route modules and app-level composition for routing; do not introduce Next.js/App Router conventions.
- Use Vite-native ESM patterns and avoid CommonJS in front-end code.
- Use shadcn/source components and `@slab/components` conventions before hand-rolled component markup.
- Keep page-specific components, hooks, stores, types, and utils colocated with the page feature. Promote code to shared folders only after real reuse exists.
- Avoid barrel files for feature exports because they can hurt Vite tree-shaking and module tracing.

## Rule categories

Read individual files in `rules/` when the task touches the relevant area:

- `async-*`: eliminate waterfalls and start independent async work early.
- `bundle-*`: keep Vite bundles statically analyzable and avoid broad imports.
- `client-*`: keep client-side storage/listeners/data-fetching lightweight.
- `rerender-*`: reduce avoidable React re-renders and stale closure bugs.
- `rendering-*`: improve rendering, hydration, transitions, and resource hints.
- `js-*`: use efficient JavaScript patterns where they materially matter.
- `advanced-*`: apply newer React patterns only when the project supports them.

## Companion skills

- Use `front-end-project-organization` when creating folders, moving files, or deciding shared vs page-local placement.
- Use `react-query-api-hooks` when reasoning about server state, query invalidation, optimistic updates, or test setup behind generated `api.useXXX` hooks.
- Use `shadcn` when adding, fixing, or composing UI components.
- Use `vite` when editing `vite.config.ts`, Vite plugins, build behavior, assets, env, or SSR/library mode.
- Use `vitest` when writing or updating front-end tests, mocks, fixtures, coverage, or test filtering.

## Full compiled reference

For the inherited React performance guide, read `AGENTS.md`. It came from the Vercel React best-practices skill and has been adapted here with repo-specific API, Vite, React Router, and shadcn constraints.
