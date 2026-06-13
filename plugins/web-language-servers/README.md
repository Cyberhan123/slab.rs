# web-language-servers

Build-only workspace package for bundled web language servers.

## Role

`plugins/web-language-servers` bundles built-in TypeScript/JavaScript, JSON, CSS/LESS/SCSS, and HTML language-server entries for the workspace editor. Its Vite build emits ESM files to:

`bin/slab-app/src-tauri/resources/libs/language-servers/web/`

This directory is not a user-installable plugin and must not contain a `plugin.json`. `crates/slab-app-core` launches these entries through `bin/slab-js-runtime lsp --entry <bundle> -- <args>`.

## Type

Bun-managed build package.

## Commands

Run from the repo root:

```sh
bun run build:language-servers
```

Package-local command:

```sh
bun run --cwd plugins/web-language-servers build
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
