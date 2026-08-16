---
name: front-end-project-organization
description: Use when creating, moving, or reviewing React Router + Vite front-end files, page-feature folders, shared components, hooks, stores, utils, route modules, layouts, and import boundaries in this repo.
---

# Front-end Project Organization

Use this skill before changing front-end file layout, adding new pages, deciding whether code belongs in shared folders, or extracting components/hooks.

## Default structure

- Keep most product code under `src/pages/<feature>` for page-owned features.
- Use `src/routes` for React Router route modules, route config, guards, and route entrypoints.
- Use `src/layouts` for app/layout shells composed by routes.
- Use shared `src/components`, `src/hooks`, `src/lib`, `src/utils`, `src/types`, `src/stores`, `src/config`, and `src/assets` only for code that is genuinely shared.
- Within a page feature, create only the folders the feature needs: `components`, `hooks`, `stores`, `types`, `utils`, or local `lib` helpers.

## Import boundaries

- Shared code can be imported by pages and app-level composition.
- Page features should import from shared code, not from sibling page features.
- Compose multiple page features at the app/route level instead of creating cross-feature imports.
- Avoid barrel files for page features. Prefer direct imports to preserve Vite tree-shaking and static analysis.

## Component placement

- Co-locate components, hooks, state, and helpers as close as possible to where they are used.
- Promote a page-local component to shared only after multiple features actually use it.
- Extract a component when a UI unit becomes independently understandable; avoid nested render functions inside large components.
- Split components that accept too many props; prefer composition with `children`, slots, or smaller components.

## Styling and UI

- Use the `shadcn` skill for component composition and shadcn-specific rules.
- Use `@slab/components` and local shared UI wrappers before creating new component primitives.
- Wrap third-party components at the boundary when the app needs stable styling, routing, or behavior adaptations.

## References

- Read `references/project-structure.md` for the original folder layout notes.
- Read `references/components-and-styling.md` for component extraction, colocation, and shared component guidance.
