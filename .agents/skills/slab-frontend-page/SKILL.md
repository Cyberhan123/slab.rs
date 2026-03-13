---
name: slab-frontend-page
description: Build or refactor routed frontend pages in slab-app. Use for work in slab-app/src/pages, routes, layouts, and page-local hooks/components. Routes to the right design, performance, and Tauri guidance for this repository.
---

# Slab Frontend Page

Use this skill for page work in `slab-app`, especially:

- `slab-app/src/pages/**`
- `slab-app/src/routes/index.tsx`
- `slab-app/src/layouts/**`
- page-local hooks, local state, and route composition

## Repo Defaults

- The app stack is React 19 + Vite + React Router 7 + Tauri 2.
- Server state uses TanStack Query with `openapi-fetch` and `openapi-react-query`.
- Client state uses Zustand.
- Shared UI primitives live in `slab-app/src/components/ui`.
- AI-heavy surfaces use Ant Design X and `antd`.

## Workflow

1. Read the current route, page, and nearby components before designing changes.
2. Preserve the existing stack unless the user explicitly asks for a technology change.
3. Keep page-specific logic close to the page. Only move code into shared UI primitives when it is reusable across screens.
4. If the task is visually ambitious, also open `../frontend-design/SKILL.md`.
5. If you want structured design-system guidance first, also open `../ui-ux-pro-max/SKILL.md`.
6. If the task touches shared UI primitives, forms, or theme variables, also open `../slab-ui-primitives/SKILL.md`.
7. If the task touches IPC, permissions, plugins, sidecars, or desktop-only behavior, also open `../slab-tauri-app/SKILL.md`.
8. After implementation, use `../vercel-react-best-practices/SKILL.md` for a repo-compatible React performance pass.

## Avoid

- Do not treat `slab-app` like a Next.js app.
- Do not introduce Server Actions, RSC-only patterns, or `next/dynamic`.
- Do not replace React Query, Zustand, Ant Design X, or the shared UI primitives without a clear reason.
- Do not create duplicate component systems when a page-local component or shared primitive already fits.

## Useful Files

- `slab-app/src/App.tsx`
- `slab-app/src/routes/index.tsx`
- `slab-app/src/lib/api/index.ts`
- `slab-app/src/store/useAppStore.ts`
- `slab-app/src/styles/globals.css`
- `slab-app/src/components/ui/**`
- `slab-app/src/pages/chat/**`

## Done When

- The page fits existing routing and state patterns.
- Shared pieces are reused through existing primitives when appropriate.
- Tauri-specific behavior is handled explicitly rather than assumed.
- Visual direction, interaction quality, and performance all match the current app.
