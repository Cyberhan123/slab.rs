# @slab/plugin-ui

Stable React UI ABI for Slab WebView plugins.

## Role

This package intentionally exposes only a safe subset of `@slab/components` plus plugin-scoped global styles. Plugin authors should import `@slab/plugin-ui/globals.css` in their Vite entry and rely on `@slab/plugin-sdk` theme mirroring for host token values.

`@slab/plugin-ui` is the stable component surface for third-party plugin UIs. Do not expose the full `@slab/components` API here unless the plugin ABI is intentionally expanded.

## Type

Bun-managed frontend package.

## Testing

Run focused checks with:

```sh
bun run --cwd packages/slab-plugin-ui build
bun run --cwd packages/slab-plugin-ui lint
```

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).

