import { describe, expect, it } from "vitest";

import {
  buildCoverageGroupName,
  createCoverageMetric,
  formatPercent,
  summarizeCommand,
  trimOutput,
} from "./utils.ts";

describe("reporter utils", () => {
  it("formats commands and coverage percentages", () => {
    expect(summarizeCommand(["cargo", "test", "--workspace"])).toBe("cargo test --workspace");
    expect(formatPercent()).toBe("n/a");
    expect(formatPercent({ count: 2, covered: 1, percent: 50 })).toBe("50.0%");
  });

  it("trims long command output and preserves short output", () => {
    expect(trimOutput("  short output  ")).toBe("short output");

    const trimmed = trimOutput("abcdef", 4);
    expect(trimmed).toBe("abcd\n\n... (2 more characters omitted)");
  });

  it("groups coverage files by top-level workspace area", () => {
    expect(buildCoverageGroupName("C:/repo", "C:/repo/crates/slab-utils/src/lib.rs")).toBe(
      "crates/slab-utils",
    );
    expect(buildCoverageGroupName("C:/repo", "C:/repo/scripts/check.ts")).toBe("scripts");
  });

  it("treats zero-count coverage as fully covered", () => {
    expect(createCoverageMetric(0, 0)).toEqual({
      count: 0,
      covered: 0,
      percent: 100,
    });
  });
});
