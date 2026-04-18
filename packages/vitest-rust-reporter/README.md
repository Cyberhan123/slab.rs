# `@slab/vitest-rust-reporter`

Workspace helper package that adapts Rust `cargo test` and optional
`cargo llvm-cov` output into a dedicated Vitest project so the results are
visible in `vitest --ui`.

## What it does

- runs a configurable Rust test command once per Vitest execution
- turns Rust unit-test results into normal Vitest suites and tests
- optionally runs a Rust coverage command and exposes summary metrics as
  additional Vitest tests
- degrades gracefully when `cargo llvm-cov` is not installed

## Default slab.rs wiring

The workspace registers `packages/vitest-rust-reporter/vitest.config.ts` as the
`rust` project in the root `vitest.config.ts`.

Default commands:

- tests: `cargo test --workspace --color never`
- coverage: `cargo llvm-cov --workspace --json --summary-only --color never`

## Notes

- rerunning a single failed Vitest task still reruns the configured Rust command,
  because the Rust results are collected externally and then projected into the
  Vitest task tree
- coverage summary support is automatic only when `cargo llvm-cov` is available
