---
name: react-query-api-hooks
description: Use when implementing or reviewing server-state data fetching in this React Router + Vite repo, especially generated api.useQuery/api.useMutation/api.useXXX hooks, TanStack Query query keys, invalidation, optimistic updates, caching, retries, suspense, testing, and mutations.
---

# React Query API Hooks

Use this skill for server state. This repo wraps TanStack Query behind generated `api.useXXX` hooks, so start from the generated API surface instead of raw `useQuery`/`useMutation` in product code.

## Repo defaults

- Prefer `api.useQuery`, `api.useMutation`, and other generated `api.useXXX` hooks for slab-server routes.
- Do not hand-write `fetch` calls in React components for generated API endpoints.
- Do not bypass the generated API types. Run `bun run gen:api` when backend API shapes change.
- Keep query variables serializable and represented in the generated hook inputs or query keys.
- Co-locate page-specific data hooks with the page feature unless they are reused across multiple features.
- Use shared invalidation helpers or generated API utilities when they exist; only reach for raw `queryClient` APIs when the generated wrapper does not cover the behavior.

## TanStack Query guidance

The copied official docs live in `references/`. Load only the files needed for the task:

- `references/overview.md` and `references/quick-start.md`: core mental model.
- `references/guides/query-keys.md`: query key uniqueness and variables.
- `references/guides/queries.md`: read/query behavior.
- `references/guides/mutations.md`: writes and mutation lifecycle.
- `references/guides/query-invalidation.md`: invalidating stale data.
- `references/guides/invalidations-from-mutations.md`: invalidation after writes.
- `references/guides/optimistic-updates.md`: optimistic UI updates.
- `references/guides/dependent-queries.md`: dependent server-state reads.
- `references/guides/parallel-queries.md`: independent reads.
- `references/guides/paginated-queries.md` and `references/guides/infinite-queries.md`: list pagination.
- `references/guides/testing.md`: test setup for query behavior.
- `references/reference/useQuery.md`, `references/reference/useMutation.md`, and `references/reference/useQueryClient.md`: low-level API details when debugging wrappers.

## Implementation checklist

- Check the generated API hook signature before adding local wrappers.
- Keep loading, empty, error, and optimistic states explicit in UI.
- Invalidate the narrowest relevant query scope after mutations.
- Avoid duplicating server state in client stores unless a UI workflow truly needs derived local state.
- Prefer selector/derived data patterns over effects that copy query data into component state.
