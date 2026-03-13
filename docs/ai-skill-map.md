# AI Skill Map

This file maps slab.rs tasks to the local skills under `.agents/skills`.

## Project Skills

- `slab-frontend-page`
  Use for routed page work in `slab-app/src/pages`, `routes`, and `layouts`.
- `slab-ui-primitives`
  Use for shared UI primitives, form building blocks, and theme-level frontend patterns.
- `slab-tauri-app`
  Use for `src-tauri`, capabilities, sidecars, plugin webviews, and desktop security boundaries.
- `slab-server-feature`
  Use for `slab-server` API, service, state, persistence, and gateway-side feature work.
- `slab-runtime-async`
  Use for async runtime internals in `slab-core` and `slab-runtime`.
- `slab-ui-review`
  Use for project-specific UI review and pre-merge UX/accessibility checks.

## General Skills To Reference Through Project Skills

- `frontend-design`
  Use through `slab-frontend-page` when the task needs a stronger visual direction.
- `ui-ux-pro-max`
  Use through `slab-frontend-page` when you want a design-system-first approach.
- `shadcn-ui`
  Use through `slab-ui-primitives` for generic primitive patterns.
- `tauri-v2`
  Use through `slab-tauri-app` for Tauri API details and common pitfalls.
- `rust-async-patterns`
  Use through `slab-runtime-async` for generic Tokio and concurrency guidance.
- `vercel-react-best-practices`
  Use through `slab-frontend-page` or `slab-ui-review` for repo-compatible React performance guidance.
- `web-design-guidelines`
  Use through `slab-ui-review` for the local review baseline and optional upstream comparison.

## Selection Order

- Page feature in `slab-app`: `slab-frontend-page`
- Shared component or theme work: `slab-ui-primitives`
- Tauri host or permission work: `slab-tauri-app`
- HTTP gateway feature in `slab-server`: `slab-server-feature`
- Runtime orchestration or backend concurrency: `slab-runtime-async`
- UI review: `slab-ui-review`
