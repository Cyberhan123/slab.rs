# slab-plugin

Framework-agnostic plugin registry and backend-neutral runtime dispatch for slab.

Owned here:

- plugin manifest loading and integrity validation;
- Wasm dispatch (`extism`);
- frontend-only fallback (non-callable).

Not owned here:

- JavaScript execution. `bin/slab-js-runtime` is a supervised sidecar owned by
  `slab-app-core`/`slab-server`, so Deno runtime details do not leak into this
  crate.
