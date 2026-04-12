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
- Default target server: `http://127.0.0.1:3000`
- Override base URL with `SLAB_SERVER_BASE_URL`

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