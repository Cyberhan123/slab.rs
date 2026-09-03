---
title: Build Workflow Guide
---

# Build Workflow Guide

Slab uses Bun and Cargo as the repository build entrypoints. Run commands from the
repository root unless a subproject README says otherwise. Do not use Bazel or
cargo-make wrappers for the top-level build flow.

## Prerequisites

Besides the Rust stable toolchain and Bun, the native dependency graph needs two
system packages on developer machines. Without them `bun run dev` fails during
`build:sidecars`.

### LLVM / Clang (libclang)

`bindgen` runs at build time for native dependencies, so `libclang` must be
installed:

- `libsqlite3-sys` compiles with `buildtime_bindgen` because `slab-js-runtime`
  pulls `deno_cache`, which enables rusqlite's `session` feature.
- `ffmpeg-sys-next` generates its own bindings with `bindgen`.

Install it per platform:

- Windows: `winget install -e --id LLVM.LLVM` (or `choco install llvm`). The
  default `C:\Program Files\LLVM\bin` location is detected automatically; for any
  other location set `LIBCLANG_PATH` to the directory containing `libclang.dll`.
- macOS: Xcode Command Line Tools (`xcode-select --install`) or `brew install llvm`.
- Linux: `sudo apt install libclang-dev` (or your distribution's equivalent).

Without it the build fails with
`Unable to find libclang: ... set the LIBCLANG_PATH environment variable`.

### FFmpeg development libraries

`slab-app-core` links FFmpeg natively by default (`ffmpeg-next-static` feature;
see `crates/slab-app-core/Cargo.toml`). `ffmpeg-sys-next` resolves FFmpeg
through, in order: the `FFMPEG_DIR` environment variable (`<dir>/include` plus
`<dir>/lib`), vcpkg (MSVC targets, `VCPKG_ROOT`), then pkg-config.

- Windows (MSVC): `vcpkg install ffmpeg:x64-windows-static-md` with `VCPKG_ROOT`
  pointing at your vcpkg checkout. The `x64-windows-static-md` triplet keeps
  Rust's default dynamic CRT while providing static FFmpeg libraries, and it is
  the default triplet the `vcpkg` crate probes, so no extra environment
  variables are needed. The installed FFmpeg major version must match the
  `ffmpeg-next` crate major version (currently 8.x); vcpkg classic mode always
  installs the newest port, so if `ports/ffmpeg/vcpkg.json` in your checkout is
  newer, pin the registry first, for example
  `git -C %VCPKG_ROOT% checkout <commit-with-ffmpeg-8.x>` before installing.
- Linux/macOS: install the libav development packages so pkg-config can find
  them (`libavcodec-dev`, `libavdevice-dev`, `libavfilter-dev`,
  `libavformat-dev`, `libavutil-dev`, `libswresample-dev`, `libswscale-dev` on
  Debian/Ubuntu; `brew install ffmpeg` on macOS).

## Daily Commands

```sh
# Install JavaScript dependencies (also runs vendor patch setup via prepare)
bun install

# Development
bun run dev
bun run dev:desktop
bun run dev:server
bun run dev:desktop:ui

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
bun run build:desktop:debug
bun run build:desktop:ui
bun run build:language-servers
bun run build:windows-installer
```

`bun run build:sidecars` compiles `slab-server`, `slab-runtime`,
`slab-js-runtime`, and `slab-python-runtime`, then stages them under
`bin/slab-app/src-tauri/binaries/` for Tauri `externalBin` packaging.
`bun run build:desktop` is the one-command desktop build: on Windows it chains
`build:windows-installer` (release sidecars + resource-less NSIS bundle),
builds `slab-windows-full-installer`, and runs `pack` to wrap the NSIS setup
with runtime payload CABs into
`target/release/bundle/nsis/Slab_<version>_x64-offline-setup.exe` — install
with that file, not the inner `*-setup.exe`. On other platforms it falls back
to the unbundled debug binary (`build:desktop:debug`), which is also what the
e2e harness uses on every platform.

`bun run dev` is the canonical full development stack alias for `bun run dev:desktop`
(Tauri spawns `slab-server` itself). `bun run dev:server` builds and runs the
headless `slab-server` alone from `target/debug/` (default bind `127.0.0.1:3000`),
for browser/mobile/remote clients; `bun run dev:desktop:ui` starts only the
desktop frontend Vite server.
`bun run test:e2e` is the only root E2E entrypoint; it owns starting that full
dev stack, waiting for the desktop UI and server `/health`, running
`packages/slab-desktop/tests/e2e`, and cleaning up the spawned process tree.
Browser-mode component and visual tests remain under `bun run test:browser`.
Run `bun run build:desktop:ui` before `bun run check:bundle-budget`; the budget
script reads `packages/slab-desktop/dist` and enforces the Plan F desktop main
chunk budget while reporting workspace chunk baselines.

## Generated Assets

```sh
bun run gen:api
bun run gen:harness
bun run gen:schemas
bun run gen:plugin-packs
bun run gen:model-packs
bun run gen:mobile
```

When backend `/v1/*` API shapes change, regenerate
`packages/api/src/v1.d.ts` with `bun run gen:api`.
When harness wire contracts (`crates/slab-proto/src/harness`, ts-rs bindings)
change, regenerate `packages/api/src/harness/` with `bun run gen:harness`;
CI diffs all three ends and fails on drift.
`bun run gen:mobile` re-exports design tokens and locales into
`flutter/slab-mobile` (see that package's README).

## Vendored Patch Workflow

Patched crates are materialized into `vendor/` by:

```sh
bun run scripts/apply-patches.ts
```

This command is executed by `bun install` through the root `prepare` script and
should also be run in CI before Cargo commands that rely on `[patch.crates-io]`.
