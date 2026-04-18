import { describe, expect, test } from "vitest";
import type { RustCoverageGroup, RustTestCase } from "../types.ts";
import { formatPercent, summarizeCommand } from "../utils.ts";
import { getRustVitestUiReport } from "./report.ts";

const uiReport = await getRustVitestUiReport();

describe("rust workspace", () => {
  test(
    `summary - ${uiReport.execution.report.summary.passed} passed, ${uiReport.execution.report.summary.failed} failed, ${uiReport.execution.report.summary.ignored} ignored`,
    () => {
      if (uiReport.execution.infrastructureError) {
        throw new Error(uiReport.execution.infrastructureError);
      }

      expect(true).toBe(true);
    },
  );

  if (uiReport.execution.report.summary.total === 0) {
    test("no rust tests discovered", () => {
      if (uiReport.execution.infrastructureError) {
        throw new Error(uiReport.execution.infrastructureError);
      }

      expect(true).toBe(true);
    });
  }

  for (const suite of uiReport.execution.report.suites) {
    describe(suite.name, () => {
      for (const testCase of suite.tests) {
        registerRustTestCase(testCase);
      }
    });
  }

  describe("coverage", () => {
    if (!uiReport.coverage.enabled) {
      test.skip("coverage disabled", () => {});
      return;
    }

    if (!uiReport.coverage.available || !uiReport.coverage.summary) {
      test.skip(
        `coverage unavailable - ${uiReport.coverage.unavailableReason ?? "no data"}`,
        () => {},
      );
      return;
    }

    const totals = uiReport.coverage.summary.totals;
    test(
      [
        "totals",
        `lines ${formatPercent(totals.lines)}`,
        `functions ${formatPercent(totals.functions)}`,
        `regions ${formatPercent(totals.regions)}`,
        `branches ${formatPercent(totals.branches)}`,
      ].join(" | "),
      () => {
        expect(true).toBe(true);
      },
    );

    for (const group of uiReport.coverage.summary.groups.slice(
      0,
      uiReport.options.ui.maxCoverageGroups,
    )) {
      registerCoverageGroup(group);
    }

    if (uiReport.coverage.command) {
      test(
        `coverage command - ${summarizeCommand(uiReport.coverage.command.command)}`,
        () => {
          expect(uiReport.coverage.command?.ok).toBe(true);
        },
      );
    }
  });
});

function registerRustTestCase(testCase: RustTestCase): void {
  const task = testCase.status === "ignored" ? test.skip : test;

  task(testCase.name, () => {
    if (testCase.status !== "failed") {
      expect(true).toBe(true);
      return;
    }

    throw new Error(testCase.output ?? "Rust test failed.");
  });
}

function registerCoverageGroup(group: RustCoverageGroup): void {
  test(
    [
      group.name,
      `lines ${formatPercent(group.metrics.lines)}`,
      `functions ${formatPercent(group.metrics.functions)}`,
      `regions ${formatPercent(group.metrics.regions)}`,
      `branches ${formatPercent(group.metrics.branches)}`,
    ].join(" | "),
    () => {
      expect(true).toBe(true);
    },
  );
}
