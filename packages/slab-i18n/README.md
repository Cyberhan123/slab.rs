# @slab/i18n

Shared internationalization package for Slab.

## Role

`@slab/i18n` provides the i18next configuration and locale resources used across Slab frontend packages. Locale content is organized by frontend page domains, for example `pages/assistant` and `pages/settings`, so feature UI copy can live with a predictable key structure.

The package exports a pre-configured i18next instance, re-exports `react-i18next` helpers, and owns frontend language preference handling so consuming packages such as `@slab/desktop` do not need to configure i18next independently.

Server-originated user-facing fields should stay key-based on the backend and be translated through frontend helpers in this package or its consumers.

## Stack

- i18next
- react-i18next
- TypeScript

## Type

Bun-managed frontend package.

## Testing

Run focused linting with:

```sh
bun run --cwd packages/slab-i18n lint
```

Run locale integrity tests with:

```sh
bun run --cwd packages/slab-i18n test:run
```

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
