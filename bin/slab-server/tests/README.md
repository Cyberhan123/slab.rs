# Server tests

This directory contains the Vitest suite for `slab-server`.

## Test groups

- Unit tests: `unit/**/*.unit.test.ts`
- Integration tests: `integration/**/*.integration.test.ts`
- Smoke tests: `smoke/**/*.smoke.test.ts`

The shared harness starts an isolated `slab-server` with temp settings, database,
and model config directories. It writes a minimal V2 `settings.json`, binds the
server to a test-selected port, and disables managed GGML children so default
route coverage stays offline and deterministic.

Smoke tests may also target an existing server:

```sh
SLAB_SERVER_BASE_URL=http://127.0.0.1:3000 bun run test:smoke
```

## Commands

Run the full Vitest project from the repo root:

```sh
bun run test:server
```

Run the local suite from this directory:

```sh
bun run test
bun run test:unit
bun run test:integration
bun run test:smoke
```

Watch mode:

```sh
bun run test:unit:watch
bun run test:integration:watch
bun run test:smoke:watch
```

## Smoke policy

The smoke suite covers the current `/v1/*` API boundary and `/health`. Runtime
heavy routes assert stable validation, not-found, or error-envelope behavior
instead of downloading models or requiring inference.

Every documented current method/path in `/api-docs/openapi.json` must have either
an executable smoke case or an explicit `it.todo` marker. Legacy llama-server
compatibility scenarios that are not implemented in Slab are represented as
future `/v1/*` TODO smoke presets rather than non-`/v1` routes.
