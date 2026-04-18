import { describe, expect, it } from "vitest";
import {
  parseCargoTestOutput,
  parseRustCoverageSummary,
} from "./index.ts";

describe("parseCargoTestOutput", () => {
  it("projects cargo test output into suites and test cases", () => {
    const output = [
      "Running unittests src/lib.rs (target/debug/deps/example.exe)",
      "running 2 tests",
      "test parser::works ... ok",
      "test parser::fails ... FAILED",
      "",
      "failures:",
      "",
      "---- parser::fails stdout ----",
      "boom",
      "",
      "test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s",
      "",
      "Doc-tests example",
      "",
      "running 1 test",
      "test src/lib.rs - docs::sample (line 10) ... ignored",
      "",
      "test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s",
    ].join("\n");

    const report = parseCargoTestOutput(output);

    expect(report.summary).toEqual({
      passed: 1,
      failed: 1,
      ignored: 1,
      total: 3,
    });
    expect(report.suites).toHaveLength(2);
    expect(report.suites[0]?.name).toBe("unittests src/lib.rs");
    expect(report.suites[0]?.tests[1]?.output).toContain("boom");
    expect(report.suites[1]?.name).toBe("Doc-tests example");
    expect(report.suites[1]?.tests[0]?.status).toBe("ignored");
  });
});

describe("parseRustCoverageSummary", () => {
  it("aggregates llvm-cov summaries by top-level workspace area", () => {
    const rawJson = JSON.stringify({
      data: [
        {
          totals: {
            lines: { count: 10, covered: 8 },
            functions: { count: 4, covered: 3 },
          },
          files: [
            {
              filename: "C:/repo/crates/slab-agent/src/lib.rs",
              summary: {
                lines: { count: 6, covered: 5 },
                functions: { count: 2, covered: 2 },
              },
            },
            {
              filename: "C:/repo/bin/slab-server/src/main.rs",
              summary: {
                lines: { count: 4, covered: 3 },
                functions: { count: 2, covered: 1 },
              },
            },
          ],
        },
      ],
    });

    const summary = parseRustCoverageSummary(rawJson, "C:/repo");

    expect(summary.totals.lines?.percent).toBe(80);
    expect(summary.groups).toEqual([
      {
        name: "bin/slab-server",
        metrics: {
          lines: { count: 4, covered: 3, percent: 75 },
          functions: { count: 2, covered: 1, percent: 50 },
        },
      },
      {
        name: "crates/slab-agent",
        metrics: {
          lines: {
            count: 6,
            covered: 5,
            percent: 83.33333333333334,
          },
          functions: { count: 2, covered: 2, percent: 100 },
        },
      },
    ]);
  });
});
