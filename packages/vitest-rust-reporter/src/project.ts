import { fileURLToPath } from "node:url";
import type {
  ResolvedRustVitestProjectOptions,
  RustVitestProjectOptions,
} from "./types.ts";

const runtimeTestFile = fileURLToPath(new URL("./runtime/rust.test.ts", import.meta.url));

export function createRustVitestProject(
  options: RustVitestProjectOptions = {},
) {
  const resolvedOptions = resolveRustVitestProjectOptions(options);

  return {
    test: {
      name: resolvedOptions.name,
      environment: "node",
      fileParallelism: false,
      isolate: false,
      testTimeout: resolvedOptions.testTimeout,
      hookTimeout: resolvedOptions.testTimeout,
      include: [runtimeTestFile],
      env: {
        VITEST_RUST_REPORTER_OPTIONS: JSON.stringify(resolvedOptions),
      },
    },
  };
}

export function resolveRustVitestProjectOptions(
  options: RustVitestProjectOptions,
): ResolvedRustVitestProjectOptions {
  return {
    cwd: options.cwd ?? process.cwd(),
    name: options.name ?? "rust",
    testCommand: options.testCommand ?? [
      "cargo",
      "test",
      "--workspace",
      "--color",
      "never",
    ],
    testTimeout: options.testTimeout ?? 15 * 60 * 1000,
    env: options.env ?? {},
    coverage: {
      enabled: options.coverage?.enabled ?? "if-available",
      command:
        options.coverage?.command ?? [
          "cargo",
          "llvm-cov",
          "--workspace",
          "--json",
          "--summary-only",
          "--color",
          "never",
        ],
    },
    ui: {
      maxCoverageGroups: options.ui?.maxCoverageGroups ?? 12,
    },
  };
}
