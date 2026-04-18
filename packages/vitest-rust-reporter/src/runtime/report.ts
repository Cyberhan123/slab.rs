import { runCommand } from "../command.ts";
import { parseCargoTestOutput, parseRustCoverageSummary } from "../parser.ts";
import type {
  ResolvedRustVitestProjectOptions,
  RustUiCoverageResult,
  RustUiExecutionResult,
  RustVitestUiReport,
} from "../types.ts";
import { summarizeCommand, trimOutput } from "../utils.ts";

let reportPromise: Promise<RustVitestUiReport> | undefined;

export async function getRustVitestUiReport(): Promise<RustVitestUiReport> {
  reportPromise ??= createRustVitestUiReport(loadResolvedOptions());
  return reportPromise;
}

function loadResolvedOptions(): ResolvedRustVitestProjectOptions {
  const rawOptions = process.env.VITEST_RUST_REPORTER_OPTIONS;
  if (!rawOptions) {
    throw new Error("Missing VITEST_RUST_REPORTER_OPTIONS.");
  }

  return JSON.parse(rawOptions) as ResolvedRustVitestProjectOptions;
}

async function createRustVitestUiReport(
  options: ResolvedRustVitestProjectOptions,
): Promise<RustVitestUiReport> {
  const execution = await runExecutionCommand(options);
  const coverage = await runCoverageCommand(options, execution);

  return {
    options,
    execution,
    coverage,
  };
}

async function runExecutionCommand(
  options: ResolvedRustVitestProjectOptions,
): Promise<RustUiExecutionResult> {
  const command = await runCommand(options.testCommand, {
    cwd: options.cwd,
    env: options.env,
    timeoutMs: options.testTimeout,
  });

  const combinedOutput = [command.stdout, command.stderr]
    .filter(Boolean)
    .join("\n");
  const report = parseCargoTestOutput(combinedOutput);
  const hasParsedTests = report.summary.total > 0;

  return {
    command,
    report,
    infrastructureError:
      !hasParsedTests && !command.ok
        ? [
            command.errorMessage ??
              `Rust command failed: ${summarizeCommand(options.testCommand)}`,
            trimOutput(combinedOutput),
          ]
            .filter(Boolean)
            .join("\n\n")
        : undefined,
  };
}

async function runCoverageCommand(
  options: ResolvedRustVitestProjectOptions,
  execution: RustUiExecutionResult,
): Promise<RustUiCoverageResult> {
  if (options.coverage.enabled === false || options.coverage.command == null) {
    return {
      enabled: false,
      available: false,
      unavailableReason: "Coverage disabled in reporter options.",
    };
  }

  if (execution.infrastructureError) {
    return {
      enabled: true,
      available: false,
      unavailableReason: "Rust test command did not complete successfully.",
    };
  }

  const command = await runCommand(options.coverage.command, {
    cwd: options.cwd,
    env: options.env,
    timeoutMs: options.testTimeout,
  });

  if (!command.ok) {
    const combinedOutput = [command.stdout, command.stderr]
      .filter(Boolean)
      .join("\n");
    const missingTool =
      combinedOutput.includes("no such command: `llvm-cov`") ||
      combinedOutput.includes("no such command: llvm-cov");

    if (options.coverage.enabled === "if-available" && missingTool) {
      return {
        enabled: true,
        available: false,
        command,
        unavailableReason: "cargo llvm-cov is not installed in this environment.",
      };
    }

    return {
      enabled: true,
      available: false,
      command,
      unavailableReason: [
        command.errorMessage ?? "Rust coverage command failed.",
        trimOutput(combinedOutput),
      ]
        .filter(Boolean)
        .join("\n\n"),
    };
  }

  return {
    enabled: true,
    available: true,
    command,
    summary: parseRustCoverageSummary(command.stdout, options.cwd),
  };
}
