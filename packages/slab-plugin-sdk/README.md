# @slab/plugin-sdk

Stable plugin-author SDK for Slab plugin webviews.

## Role

`@slab/plugin-sdk` is the browser-facing SDK for third-party and local plugin UIs. It provides:

- A typed host bridge for plugin webviews running inside the Tauri child WebView sandbox.
- A direct-HTTP Slab API client built on `@slab/api`, guarded client-side by the plugin Slab API surface. Plugins call slab-server directly (`slab-plugin-sdk → @slab/api → slab-server`); the desktop host no longer forwards plugin HTTP.
- The plugin Slab API surface definition (`SLAB_API_PERMISSIONS`, labels, `requiredSlabApiPermission`) — this lives here, not in `@slab/api`.
- Theme snapshot types and document helpers so plugin UIs can mirror host tokens.
- Integrity-related exports used by plugin packaging and verification flows.
- A browser bundle export for non-workspace consumers.

## Type

Bun-managed frontend package.

## Testing

- Type-check and build the browser bundle with `bun run build`.
- Rebuild only the browser bundle with `bun run build:browser`.
- Run tests with `bun run test`.
- Run the non-watch test suite with `bun run test:run`.

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).