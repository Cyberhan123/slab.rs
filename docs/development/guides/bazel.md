---
title: Bazel Usage Guide
---

# Bazel Usage Guide

Slab uses Bazelisk as the repository build entrypoint. The root `package.json`
scripts are convenience wrappers around Bazel targets, while Bazel targets keep
the build, check, generation, packaging, and development entrypoints in one
place.

Cargo remains the single source of truth for Rust dependencies, versions, and
features. Bazel consumes that graph through Bzlmod and `crate_universe`, then
provides a hermetic command surface for CI, cross-platform builds, and cached
workspace automation.

## Responsibilities

Bazel owns the top-level command surface:

- Build and package entrypoints for the desktop app and Windows installer.
- Workspace check and test entrypoints.
- Generated API, schema, and plugin-pack assets.
- Rust dependency graph rendering through Bzlmod, `rules_rust`, and
  `crate_universe`.
- Runtime artifact staging and runfiles paths used by helper commands.

Some Rust flows still use Cargo under Bazel-managed wrappers. This is
intentional for flows that are not yet pure Bazel on Windows, especially
workspace-wide sidecar checks and builds.

## Daily Commands

Run these from the repository root.

```sh
# Start the main app development stack.
bun run dev:app

# Run the standard workspace checks.
bun run check
bun run check:rust
bun run lint:rust
bun run check:bazel
bun run bazel:lock:check

# Run the standard test suite.
bun run test
bun run test:bazel
bun run test:rust:bazel

# Build common deliverables.
bun run build:desktop
bun run build:app
bun run build:windows-installer
```

The equivalent direct Bazel targets are:

```sh
bazelisk run //bin/slab-app:dev
bazelisk run //tools/check:workspace
bazelisk run //tools/rust:build_sidecars
bazelisk build //tools/rust:clippy_workspace
bazelisk query //...
bazelisk run //tools/test:workspace
bazelisk test //...
bazelisk test //...
bazelisk mod deps --lockfile_mode=error
bazelisk run //tools/frontend:desktop_build
bazelisk run //bin/slab-app:bundle_debug
bazelisk run --config=windows-x86_64-gnullvm //bin/slab-app:release_windows_x86_64_gnullvm
```

Prefer the `bun run ...` form for day-to-day use and the `bazelisk ...` form
when debugging a specific Bazel target.

## Generated Assets

Use Bazel-backed generation commands so generated outputs come from the same
entrypoints used by CI and packaging.

```sh
bun run gen:api
bun run gen:schemas
bun run gen:plugin-packs
```

Direct targets:

```sh
bazelisk run //tools/gen:api
bazelisk run //tools/gen:schemas
bazelisk run //tools/plugins:plugin_packs
```

When backend `/v1/*` API shapes change, regenerate
`packages/api/src/v1.d.ts` with `bun run gen:api`.

## Lockfile Discipline

Keep `MODULE.bazel.lock` in sync whenever Cargo or Bazel module inputs change.

```sh
# Refresh the Bazel module lockfile.
bun run bazel:lock:update

# Verify the lockfile is current without rewriting it.
bun run bazel:lock:check
```

This keeps Bazel module resolution deterministic in CI while preserving Cargo as
the dependency source of truth.

## Rust Dependency Graph

Rust third-party dependencies are rendered through Bzlmod and
`rules_rust` `crate_universe` in `MODULE.bazel`.

Key files:

- `MODULE.bazel` declares Bazel modules, Rust toolchains, crate annotations,
  and supported Rust platform triples.
- `MODULE.bazel.lock` records the resolved Bazel module and crate universe
  state.
- `Cargo.toml` and `Cargo.lock` remain the Cargo source of truth used by
  `crate_universe`.
- `vendor/` contains patched Rust crates and runtime artifacts that must be
  visible to both Cargo and Bazel when they affect shared builds.

When a crate patch is needed for both Cargo and Bazel, keep the source in
`vendor/`, add the matching `[patch.crates-io]` entry in `Cargo.toml`, and make
sure `MODULE.bazel.lock` is refreshed by running a Bazel command that resolves
the crate graph.

## Rust Bazel Targets

Rust check, sidecar, and clippy targets live in `tools/rust/BUILD.bazel`.
`tools/cargo/BUILD.bazel` is kept as a compatibility alias layer for older
target names, but new commands should use `//tools/rust:*` directly.

Common targets:

```sh
bazelisk run //tools/rust:build_sidecars
bazelisk run //tools/rust:check_sidecars
bazelisk run //tools/rust:check_workspace
bazelisk build //tools/rust:clippy_workspace
```

On Windows, sidecar builds currently run Cargo from the Bazel-managed
`//tools/rust:build_sidecars` helper. The helper injects Bazel-discovered
runtime dependency paths, builds the sidecar packages, and stages Tauri
`externalBin` files under `bin/slab-app/src-tauri/binaries`. Keep
Bazel-discovered runtime path setup in `tools/bazel/workspace_command.py` when a
helper still needs to run a host command from the repository root.

## Release Targets

Release staging entrypoints live on `//bin/slab-app` and take their platform
selection from Bazel configs in `.bazelrc`.

```sh
bazelisk run --config=linux-x86_64 //bin/slab-app:release_linux_x86_64
bazelisk run --config=linux-aarch64 //bin/slab-app:release_linux_aarch64
bazelisk run --config=macos-x86_64 //bin/slab-app:release_macos_x86_64
bazelisk run --config=macos-aarch64 //bin/slab-app:release_macos_aarch64
bazelisk run --config=windows-x86_64-gnullvm //bin/slab-app:release_windows_x86_64_gnullvm
bazelisk run --config=windows-aarch64-gnullvm //bin/slab-app:release_windows_aarch64_gnullvm
```

The release targets stage artifacts under `dist/<platform>/` after Tauri
finishes packaging.

## Runfiles and Helper Commands

Helper targets that need binaries produced by other Bazel targets should depend
on those binaries through `data` and execute them from runfiles. Avoid running
`bazelisk run` from inside another Bazel-run helper unless there is no direct
runfiles path available.

This matters on Windows because nested Bazel invocations can contend for the
same output base and can restart the Bazel server when startup options differ.

## Windows Notes

The repository `.bazelrc` keeps Windows output paths short and enables symlink
and runfiles behavior needed by the current build:

```sh
startup --output_base=C:/tmp/b
startup --windows_enable_symlinks
```

If Windows developer mode or symlink permissions are unavailable, Bazel may
fall back to slower behavior or fail to materialize expected runfiles.

The FFmpeg SDK is provided through the `ffmpeg_windows_x64` external repository
and is surfaced to Cargo commands by the workspace command wrapper. If a Cargo
wrapper cannot find FFmpeg, check that Bazel has resolved external repositories
and that `C:/tmp/b/external/+http_archive+ffmpeg_windows_x64` exists.

## Updating Bazel Files

Use these rules when editing Bazel build files:

- Keep root scripts as small user-facing wrappers around Bazel targets.
- Put reusable command behavior in `tools/bazel/workspace_command.py`.
- Prefer direct target dependencies and runfiles over nested Bazel commands.
- Keep Rust cross-crate contracts in `crates/slab-types` or
  `crates/slab-proto`; do not add a parallel generated-contract path.
- When changing API contracts, regenerate the frontend API package with
  `bun run gen:api`.

Run `bun run check:bazel` after BUILD or MODULE changes, run
`bun run bazel:lock:check` after dependency graph edits, and run the narrowest
affected build/check target before broad workspace checks.
