import { describe, expect, it } from "vitest";

import {
  createRustVitestProject,
  resolveRustVitestProjectOptions,
} from "./project.ts";

describe("resolveRustVitestProjectOptions", () => {
  it("fills in the default reporter wiring", () => {
    const resolved = resolveRustVitestProjectOptions({});

    expect(resolved.cwd).toBe(process.cwd());
    expect(resolved.name).toBe("rust");
    expect(resolved.testCommand).toEqual([
      "cargo",
      "test",
      "--workspace",
      "--color",
      "never",
    ]);
    expect(resolved.coverage).toEqual({
      enabled: "if-available",
      command: [
        "cargo",
        "llvm-cov",
        "--workspace",
        "--json",
        "--summary-only",
        "--color",
        "never",
      ],
    });
    expect(resolved.ui.maxCoverageGroups).toBe(12);
  });

  it("preserves explicit project overrides", () => {
    const resolved = resolveRustVitestProjectOptions({
      coverage: {
        command: ["cargo", "llvm-cov", "--json"],
        enabled: true,
      },
      cwd: "/tmp/slab",
      env: { CARGO_TERM_COLOR: "always" },
      name: "rust-custom",
      testCommand: ["cargo", "test", "-p", "slab-utils"],
      testTimeout: 42_000,
      ui: { maxCoverageGroups: 3 },
    });

    expect(resolved).toMatchObject({
      cwd: "/tmp/slab",
      env: { CARGO_TERM_COLOR: "always" },
      name: "rust-custom",
      testCommand: ["cargo", "test", "-p", "slab-utils"],
      testTimeout: 42_000,
      coverage: {
        enabled: true,
        command: ["cargo", "llvm-cov", "--json"],
      },
      ui: { maxCoverageGroups: 3 },
    });
  });
});

describe("createRustVitestProject", () => {
  it("projects the resolved options into a Vitest project definition", () => {
    const project = createRustVitestProject({
      cwd: "/tmp/slab",
      name: "rust-ui",
      testTimeout: 30_000,
    });

    expect(project.test).toMatchObject({
      environment: "node",
      fileParallelism: false,
      hookTimeout: 30_000,
      isolate: false,
      name: "rust-ui",
      testTimeout: 30_000,
    });
    expect(project.test.include).toHaveLength(1);
    expect(project.test.env?.VITEST_RUST_REPORTER_OPTIONS).toContain('"cwd":"/tmp/slab"');
    expect(project.test.env?.VITEST_RUST_REPORTER_OPTIONS).toContain('"name":"rust-ui"');
  });
});
