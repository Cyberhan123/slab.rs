import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CommandResult,
  ResolvedRustVitestProjectOptions,
  RustCoverageSummary,
  RustExecutionReport,
} from "../types.ts";

const runCommandMock = vi.hoisted(() =>
  vi.fn<
    (
      command: string[],
      options: { cwd: string; env?: Record<string, string | undefined>; timeoutMs: number },
    ) => Promise<CommandResult>
  >(),
);
const parseCargoTestOutputMock = vi.hoisted(() =>
  vi.fn<(output: string) => RustExecutionReport>(),
);
const parseRustCoverageSummaryMock = vi.hoisted(() =>
  vi.fn<(rawJson: string, cwd: string) => RustCoverageSummary>(),
);

vi.mock("../command.ts", () => ({
  runCommand: runCommandMock,
}));

vi.mock("../parser.ts", () => ({
  parseCargoTestOutput: parseCargoTestOutputMock,
  parseRustCoverageSummary: parseRustCoverageSummaryMock,
}));

function commandResult(overrides: Partial<CommandResult> = {}): CommandResult {
  return {
    command: ["cargo", "test"],
    cwd: "/repo",
    errorMessage: undefined,
    exitCode: 0,
    ok: true,
    stderr: "",
    stdout: "",
    timedOut: false,
    ...overrides,
  };
}

function executionReport(total: number): RustExecutionReport {
  return {
    suites: [],
    summary: {
      failed: total > 0 ? 0 : 0,
      ignored: 0,
      passed: total,
      total,
    },
  };
}

function resolvedOptions(
  overrides: Partial<ResolvedRustVitestProjectOptions> = {},
): ResolvedRustVitestProjectOptions {
  return {
    coverage: {
      command: ["cargo", "llvm-cov", "--json"],
      enabled: "if-available",
    },
    cwd: "/repo",
    env: {},
    name: "rust",
    testCommand: ["cargo", "test"],
    testTimeout: 1_000,
    ui: { maxCoverageGroups: 12 },
    ...overrides,
  };
}

async function loadReportModule() {
  vi.resetModules();
  return import("./report.ts");
}

describe("getRustVitestUiReport", () => {
  beforeEach(() => {
    delete process.env.VITEST_RUST_REPORTER_OPTIONS;
    runCommandMock.mockReset();
    parseCargoTestOutputMock.mockReset();
    parseRustCoverageSummaryMock.mockReset();
  });

  afterEach(() => {
    delete process.env.VITEST_RUST_REPORTER_OPTIONS;
  });

  it("throws when reporter options are missing", async () => {
    const { getRustVitestUiReport } = await loadReportModule();

    await expect(getRustVitestUiReport()).rejects.toThrow(
      "Missing VITEST_RUST_REPORTER_OPTIONS.",
    );
  });

  it("reports infrastructure errors when the rust command fails before any tests are parsed", async () => {
    process.env.VITEST_RUST_REPORTER_OPTIONS = JSON.stringify(resolvedOptions());
    runCommandMock.mockResolvedValueOnce(
      commandResult({
        command: ["cargo", "test", "--workspace"],
        exitCode: 101,
        ok: false,
        stderr: "compile failed",
      }),
    );
    parseCargoTestOutputMock.mockReturnValue(executionReport(0));

    const { getRustVitestUiReport } = await loadReportModule();
    const report = await getRustVitestUiReport();

    expect(report.execution.infrastructureError).toContain("Rust command failed: cargo test");
    expect(report.execution.infrastructureError).toContain("compile failed");
    expect(report.coverage).toEqual({
      available: false,
      enabled: true,
      unavailableReason: "Rust test command did not complete successfully.",
    });
    expect(runCommandMock).toHaveBeenCalledTimes(1);
  });

  it("treats cargo llvm-cov as optional when configured if-available", async () => {
    process.env.VITEST_RUST_REPORTER_OPTIONS = JSON.stringify(resolvedOptions());
    runCommandMock
      .mockResolvedValueOnce(commandResult())
      .mockResolvedValueOnce(
        commandResult({
          command: ["cargo", "llvm-cov"],
          errorMessage: "spawn failed",
          exitCode: 1,
          ok: false,
          stderr: "error: no such command: `llvm-cov`",
        }),
      );
    parseCargoTestOutputMock.mockReturnValue(executionReport(2));

    const { getRustVitestUiReport } = await loadReportModule();
    const report = await getRustVitestUiReport();

    expect(report.coverage).toMatchObject({
      available: false,
      enabled: true,
      unavailableReason: "cargo llvm-cov is not installed in this environment.",
    });
    expect(report.coverage.command?.stderr).toContain("llvm-cov");
  });

  it("surfaces coverage command failures when they are required", async () => {
    process.env.VITEST_RUST_REPORTER_OPTIONS = JSON.stringify(
      resolvedOptions({
        coverage: {
          command: ["cargo", "llvm-cov"],
          enabled: true,
        },
      }),
    );
    runCommandMock
      .mockResolvedValueOnce(commandResult())
      .mockResolvedValueOnce(
        commandResult({
          command: ["cargo", "llvm-cov"],
          errorMessage: "spawn failed",
          exitCode: null,
          ok: false,
          stderr: "some fatal coverage error",
        }),
      );
    parseCargoTestOutputMock.mockReturnValue(executionReport(1));

    const { getRustVitestUiReport } = await loadReportModule();
    const report = await getRustVitestUiReport();

    expect(report.coverage.available).toBe(false);
    expect(report.coverage.unavailableReason).toContain("spawn failed");
    expect(report.coverage.unavailableReason).toContain("some fatal coverage error");
  });

  it("parses coverage output when the command succeeds", async () => {
    const summary: RustCoverageSummary = {
      groups: [],
      totals: {
        lines: { count: 10, covered: 9, percent: 90 },
      },
    };
    process.env.VITEST_RUST_REPORTER_OPTIONS = JSON.stringify(resolvedOptions());
    runCommandMock
      .mockResolvedValueOnce(commandResult())
      .mockResolvedValueOnce(
        commandResult({
          command: ["cargo", "llvm-cov"],
          stdout: '{"data":[]}',
        }),
      );
    parseCargoTestOutputMock.mockReturnValue(executionReport(1));
    parseRustCoverageSummaryMock.mockReturnValue(summary);

    const { getRustVitestUiReport } = await loadReportModule();
    const report = await getRustVitestUiReport();

    expect(report.coverage).toMatchObject({
      available: true,
      enabled: true,
      summary,
    });
    expect(parseRustCoverageSummaryMock).toHaveBeenCalledWith('{"data":[]}', "/repo");
  });
});
