# Server tests

This directory keeps a legacy Python/pytest test suite and now supports
Vitest-based migration tests.

## Legacy Python tests (being phased out)

- Python unit/integration checks under `unit/`
- Run with `pytest` (or `tests.sh`)
- These are legacy compatibility checks and should be migrated to Vitest incrementally.

## Vitest tests

- Unit migration tests: `unit/**/*.unit.test.ts`
- Integration tests: `integration/**/*.integration.test.ts`
- Unit tests self-start an isolated `slab-server` with temp settings/db/model dirs
- The unit-test harness writes a minimal V2 `settings.json` that binds the HTTP server
  to the test-selected port and disables managed runtime backends, so route-level tests
  do not spend startup time launching local GGML children
- Integration tests target `http://127.0.0.1:3000` by default
- Override integration target base URL with `SLAB_SERVER_BASE_URL`

Run unit tests:

```sh
cd bin/slab-server/tests
bun run test:unit
```

Run integration tests:

```sh
cd bin/slab-server/tests
bun run test:integration
```

Watch mode:

```sh
cd bin/slab-server/tests
bun run test:unit:watch
```

```sh
cd bin/slab-server/tests
bun run test:integration:watch
```

## Migration status

- `unit/server-basics.unit.test.ts` now covers the useful legacy checks from
  `unit/test_basic.py` and `unit/test_security.py` against the current
  `slab-server` API surface.
- Remaining Python files are legacy compatibility tests and should be migrated
  selectively instead of copied 1:1 when their covered routes still matter.
