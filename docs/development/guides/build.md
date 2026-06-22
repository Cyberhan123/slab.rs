---
title: Build Workflow Guide
---

# Build Workflow Guide

Slab uses Bun and Cargo as the repository build entrypoints. Run commands from the
repository root unless a subproject README says otherwise. Do not use Bazel or
cargo-make wrappers for the top-level build flow.

## Daily Commands

```sh
# Install JavaScript dependencies (also runs vendor patch setup via prepare)
bun install

# Development
bun run dev
bun run dev:app
bun run dev:desktop

# Checks
bun run check
bun run check:frontend
bun run check:rust
bun run check:bundle-budget
bun run lint:rust

# Tests
bun run test
bun run test:frontend
bun run test:rust
bun run test:browser
bun run test:e2e

# Builds
bun run build:sidecars
bun run build:desktop
bun run build:language-servers
bun run build:app
bun run build:windows-installer
```

`bun run build:sidecars` compiles `slab-server`, `slab-runtime`,
`slab-js-runtime`, and `slab-python-runtime`, then stages them under
`bin/slab-app/src-tauri/binaries/` for Tauri `externalBin` packaging.
`bun run build:windows-installer` uses release sidecars before building the NSIS
bundle.

`bun run dev` is the canonical full development stack alias for `bun run dev:app`.
`bun run test:e2e` is the only root E2E entrypoint; it owns starting that full
dev stack, waiting for the desktop UI and server `/health`, running
`packages/slab-desktop/tests/e2e`, and cleaning up the spawned process tree.
Browser-mode component and visual tests remain under `bun run test:browser`.
Run `bun run build:desktop` before `bun run check:bundle-budget`; the budget
script reads `packages/slab-desktop/dist` and enforces the Plan F desktop main
chunk budget while reporting workspace chunk baselines.

## Generated Assets

```sh
bun run gen:api
bun run gen:schemas
bun run gen:plugin-packs
bun run gen:model-packs
```

When backend `/v1/*` API shapes change, regenerate
`packages/api/src/v1.d.ts` with `bun run gen:api`.

## Vendored Patch Workflow

Patched crates are materialized into `vendor/` by:

```sh
bun run scripts/apply-patches.ts
```

This command is executed by `bun install` through the root `prepare` script and
should also be run in CI before Cargo commands that rely on `[patch.crates-io]`.
