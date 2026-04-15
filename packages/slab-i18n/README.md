# @slab/i18n

Shared internationalization package for Slab.

## Role

`@slab/i18n` provides the i18next configuration and locale resources used across Slab frontend packages. Locale content is organized by frontend page domains (for example `pages/chat` and `pages/settings`) so feature UI copy can live with a predictable key structure. The package exports a pre-configured i18next instance, re-exports `react-i18next` helpers, and owns frontend language preference handling (`Auto`, `English`, `中文`) so consuming packages (`@slab/desktop`) do not need to configure i18next independently.

## Stack

- [i18next](https://www.i18next.com/)
- [react-i18next](https://react.i18next.com/)
- TypeScript

## Type

Bun-managed frontend package.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
