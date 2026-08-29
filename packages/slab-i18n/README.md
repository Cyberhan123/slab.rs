# @slab/i18n

Shared internationalization package for Slab.

## Role

`@slab/i18n` provides the i18next configuration and locale resources used across Slab frontend packages. Locale content is organized by frontend page domains, for example `pages/assistant` and `pages/settings`, so feature UI copy can live with a predictable key structure. Cross-page shared copy lives in the `common.*` domain (`locales/*/common.ts`) — add a key there instead of duplicating the same en+zh string across page namespaces.

The package exports a pre-configured i18next instance, re-exports `react-i18next` helpers, and owns frontend language preference handling so consuming packages such as `@slab/desktop` do not need to configure i18next independently.

Server-originated user-facing fields should stay key-based on the backend and be translated through frontend helpers in this package or its consumers.

## Adding or changing keys

1. Add the key to **both** `locales/en-US/...` and `locales/zh-CN/...` (zh is enforced by `satisfies LocaleSchema` plus the parity test; placeholders `{{like}}` must match).
2. Reference it from consumer code (`packages/slab-ui/src`) — the `unused-keys` test fails for locale keys that no source references.
3. If a key is intentionally kept without a static reference (e.g. future dynamic use), add it to `ALLOWED_UNUSED` in `src/__tests__/unused-keys.test.ts` with a reason.
4. zh-CN wording must follow the glossary in [TERMS.md](./TERMS.md).

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
