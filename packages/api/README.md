# @slab/api

Shared TypeScript API package for Slab.

## Role

`@slab/api` is the shared frontend HTTP API layer for the slab-server `/v1` surface. It is consumed by `@slab/desktop` and by `@slab/plugin-sdk`. It owns only:

- Generated OpenAPI v1 types from `src/v1.d.ts`.
- `openapi-fetch` and `openapi-react-query` client factories for the `/v1` HTTP surface.
- Shared API error helpers and model normalization utilities.

Plugin API surface definitions (the allowed `permissions.slabApi` surface, its labels, and the surface guard) live in `@slab/plugin-sdk`, not here.

Regenerate the OpenAPI contract with `bun run gen:api` from the repo root when backend API shapes change.

## Type

Bun-managed frontend package.

## Testing

- Type-check with `bun run build`.
- Run tests with `bun run test`.
- Run the non-watch test suite with `bun run test:run`.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).