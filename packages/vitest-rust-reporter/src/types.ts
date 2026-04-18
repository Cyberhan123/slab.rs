export type RustCaseStatus = "passed" | "failed" | "ignored";

export interface RustTestCase {
  name: string;
  status: RustCaseStatus;
  output?: string;
}

export interface RustSuite {
  name: string;
  tests: RustTestCase[];
}

export interface RustExecutionSummary {
  passed: number;
  failed: number;
  ignored: number;
  total: number;
}

export interface RustExecutionReport {
  suites: RustSuite[];
  summary: RustExecutionSummary;
}

export interface CoverageMetric {
  count: number;
  covered: number;
  percent: number;
}

export interface RustCoverageGroup {
  name: string;
  metrics: Partial<Record<RustCoverageMetricKey, CoverageMetric>>;
}

export interface RustCoverageSummary {
  totals: Partial<Record<RustCoverageMetricKey, CoverageMetric>>;
  groups: RustCoverageGroup[];
}

export type RustCoverageMetricKey =
  | "lines"
  | "functions"
  | "regions"
  | "branches"
  | "instantiations";

export interface RustCoverageOptions {
  enabled?: boolean | "if-available";
  command?: string[];
}

export interface RustVitestUiOptions {
  maxCoverageGroups?: number;
}

export interface RustVitestProjectOptions {
  cwd?: string;
  name?: string;
  testCommand?: string[];
  testTimeout?: number;
  env?: Record<string, string | undefined>;
  coverage?: RustCoverageOptions;
  ui?: RustVitestUiOptions;
}

export interface ResolvedRustVitestProjectOptions {
  cwd: string;
  name: string;
  testCommand: string[];
  testTimeout: number;
  env: Record<string, string | undefined>;
  coverage: {
    enabled: boolean | "if-available";
    command: string[] | null;
  };
  ui: {
    maxCoverageGroups: number;
  };
}

export interface CommandResult {
  command: string[];
  cwd: string;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  ok: boolean;
  timedOut: boolean;
  errorMessage?: string;
}

export interface RustUiExecutionResult {
  command: CommandResult;
  report: RustExecutionReport;
  infrastructureError?: string;
}

export interface RustUiCoverageResult {
  enabled: boolean;
  available: boolean;
  command?: CommandResult;
  summary?: RustCoverageSummary;
  unavailableReason?: string;
}

export interface RustVitestUiReport {
  options: ResolvedRustVitestProjectOptions;
  execution: RustUiExecutionResult;
  coverage: RustUiCoverageResult;
}
