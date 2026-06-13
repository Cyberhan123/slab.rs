# @slab/plugin-video-subtitle-translator

Built-in Slab plugin for translating video subtitles.

## Role

`plugins/video-subtitle-translator` is a manifest v1 plugin package. It contributes:

- A sandboxed WebView UI route and sidebar item.
- A JS backend entry built to `dist/plugin.js`.
- Settings and agent capability schemas under `schemas/`.
- Host-authorized calls for model loading, audio transcription, subtitle rendering, chat completion, task reads/cancelation, and scoped video/subtitle file access.

Plugin source files should keep using `@slab/plugin-sdk` for host bridge calls and `@slab/plugin-ui` for stable plugin UI primitives. Do not bypass `plugin.json` permissions or call local Slab HTTP origins directly from plugin code.

## Type

Bun-managed built-in plugin package.

## Commands

Run plugin UI development locally with:

```sh
bun run --cwd plugins/video-subtitle-translator dev
```

Build the plugin package with:

```sh
bun run --cwd plugins/video-subtitle-translator build
```

Regenerate built-in plugin archives from the repo root with:

```sh
bun run gen:plugin-packs
```

## License

AGPL-3.0-only. See [LICENSE](../../LICENSE).
