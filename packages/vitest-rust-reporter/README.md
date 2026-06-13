# @slab/vitest-rust-reporter

Workspace helper package that adapts Rust `cargo test` and optional `cargo llvm-cov` output into a dedicated Vitest project so the results are visible in `vitest --ui`.

## Role

`@slab/vitest-rust-reporter`:

- Runs a configurable Rust test command once per Vitest execution.
- Turns Rust unit-test results into normal Vitest suites and tests.
- Optionally runs a Rust coverage command and exposes summary metrics as additional Vitest tests.
- Degrades gracefully when `cargo llvm-cov` is not installed.

## Default Slab Wiring

The workspace registers `packages/vitest-rust-reporter/vitest.config.ts` as the `rust` project in the root `vitest.config.ts`.

Default commands:

- Tests: `cargo test --workspace --color never`.
- Coverage: `cargo llvm-cov --workspace --json --summary-only --color never`.

Rerunning a single failed Vitest task still reruns the configured Rust command because the Rust results are collected externally and then projected into the Vitest task tree.

## Type

Bun-managed test helper package.

## Testing

Run focused tests with:

```sh
bun run --cwd packages/vitest-rust-reporter test:run
```

## License

AGPL-3.0-only. See the root [LICENSE](../../LICENSE).
