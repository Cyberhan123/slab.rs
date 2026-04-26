# @slab/api

Shared TypeScript API package for Slab.

## Role

`@slab/api` is the shared frontend API layer used by `@slab/desktop` and plugin-facing bridge code. It provides:

- Generated OpenAPI v1 types from `src/v1.d.ts`.
- `openapi-fetch` and `openapi-react-query` client factories for the `/v1` HTTP surface.
- Shared API error helpers and model normalization utilities.
- Plugin-safe bridge transport helpers for host-mediated API access.

Regenerate the OpenAPI contract with `bun run gen:api` from the repo root when backend API shapes change.

## Type

Bun-managed frontend package.

## Testing

- Type-check with `bun run build`.
- Run tests with `bun run test`.
- Run the non-watch test suite with `bun run test:run`.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).