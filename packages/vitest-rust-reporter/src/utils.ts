import path from "node:path";
import type { CoverageMetric, RustCoverageMetricKey } from "./types.ts";

export const COVERAGE_KEYS: RustCoverageMetricKey[] = [
  "lines",
  "functions",
  "regions",
  "branches",
  "instantiations",
];

export function normalizePath(value: string): string {
  return value.replace(/\\/g, "/");
}

export function summarizeCommand(command: readonly string[]): string {
  return command.join(" ");
}

export function formatPercent(metric?: CoverageMetric): string {
  if (!metric) {
    return "n/a";
  }

  return `${metric.percent.toFixed(1)}%`;
}

export function trimOutput(value: string, maxLength = 10_000): string {
  const trimmed = value.trim();

  if (trimmed.length <= maxLength) {
    return trimmed;
  }

  const omitted = trimmed.length - maxLength;
  return `${trimmed.slice(0, maxLength)}\n\n... (${omitted} more characters omitted)`;
}

export function buildCoverageGroupName(
  cwd: string,
  filename: string,
): string {
  const relativePath = normalizePath(path.relative(cwd, filename));
  const parts = relativePath.split("/").filter(Boolean);

  if (parts.length === 0) {
    return ".";
  }

  if (
    ["bin", "crates", "packages", "plugins", "models", "docs"].includes(
      parts[0] ?? "",
    ) &&
    parts.length >= 2
  ) {
    return `${parts[0]}/${parts[1]}`;
  }

  return parts[0] ?? relativePath;
}

export function createCoverageMetric(
  covered: number,
  count: number,
): CoverageMetric {
  return {
    covered,
    count,
    percent: count > 0 ? (covered / count) * 100 : 100,
  };
}
