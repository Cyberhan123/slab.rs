# slab-js-runtime

> Some of the code comes from https://github.com/rscarson/rustyscript, but for easier dependency management and upgrades, I've moved it to src/infra/deno.

`slab-js-runtime` is the supervised JavaScript plugin execution sidecar owned
by `slab-server`. It communicates over line-delimited JSON-RPC 2.0 on stdio and
keeps Deno crate API churn out of `slab-app-core`, `slab-server`, and
`slab-plugin`.

The runtime accepts `plugin.call` requests, imports the plugin ESM entry, and
calls a named export. Entries may be `.ts`, `.tsx`, `.js`, or `.mjs`; CommonJS
`module.exports` is intentionally not supported.

Security model:

- no ambient file or network permission;
- plugin package files are readable for module loading only;
- `fetch` requires `permissions.network.mode = "allowlist"` and a matching
  host in `permissions.network.allowHosts`;
- local Slab API origins are blocked from `fetch`; use `Slab.api.request`;
- `Deno.readFile` and `Deno.writeFile` require per-call grants plus matching
  manifest file permission labels.

Host callbacks:

- `slab.api.request` is sent back to `slab-server` and re-authorized against
  `permissions.slabApi`.
- `slab.ui.emit` is sent back to `slab-server`, published on
  `/v1/plugins/events`, then forwarded by `slab-app` as
  `plugin://{pluginId}/event`.
