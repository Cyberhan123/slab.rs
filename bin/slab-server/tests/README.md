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

The server process is spawned with its working directory set to the per-run
temp directory — deliberately outside any workspace checkout — so the server's
CWD-ancestor workspace fallback (`workspace_root_from_config` in
`crates/slab-app-core`) resolves `None` and closed-workspace `400` semantics
hold (the out-of-process mirror of the in-process harness's
`set_cwd_workspace_fallback_enabled(false)`). Keep it that way; this assumes
the system temp dir is not inside a `.git`/`.slab`-marked checkout. When no
usable prebuilt `target/debug` binary exists, the harness first runs
`cargo build` from the repo root (so `.cargo/config.toml` link flags still
apply) and then spawns that binary with the temp cwd.

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

Coverage is recorded client-side by the harness (every request and WebSocket
open, before the response is inspected). The `afterAll` check asserts the
recorded set equals `executableSmokeOperations` in `smoke/server-api/shared.ts`,
and the first core test asserts the served OpenAPI document equals that list
plus `todoSmokeOperations`. A newly documented operation therefore needs BOTH
an `executableSmokeOperations` entry AND a smoke step that fires it, in the
same change; defer execution with a `todoSmokeOperations` entry instead of
leaving the map unbalanced.
