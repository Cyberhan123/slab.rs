import type {
  CoverageMetric,
  RustCoverageGroup,
  RustCoverageMetricKey,
  RustCoverageSummary,
  RustExecutionReport,
  RustSuite,
  RustTestCase,
} from "./types.ts";
import {
  buildCoverageGroupName,
  COVERAGE_KEYS,
  createCoverageMetric,
  normalizePath,
} from "./utils.ts";

const RUNNING_SUITE_PATTERN = /^Running (.+?)(?: \(.+\))?$/;
const DOC_TEST_SUITE_PATTERN = /^Doc-tests (.+)$/;
const TEST_RESULT_PATTERN = /^test (.+) \.\.\. (ok|FAILED|ignored)$/;
const FAILURE_BLOCK_PATTERN = /^---- (.+?) (stdout|stderr) ----$/;

interface LlvmCovMetricLike {
  count?: number;
  covered?: number;
  percent?: number;
}

interface LlvmCovFileLike {
  filename?: string;
  summary?: Partial<Record<RustCoverageMetricKey, LlvmCovMetricLike>>;
}

interface LlvmCovDataLike {
  files?: LlvmCovFileLike[];
  totals?: Partial<Record<RustCoverageMetricKey, LlvmCovMetricLike>>;
}

interface LlvmCovJsonLike {
  data?: LlvmCovDataLike[];
}

export function parseCargoTestOutput(output: string): RustExecutionReport {
  const normalizedOutput = output.replace(/\r\n/g, "\n");
  const lines = normalizedOutput.split("\n");
  const failureBlocks = extractFailureBlocks(lines);
  const suites: RustSuite[] = [];
  let currentSuite = getOrCreateSuite(suites, "workspace");

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }

    const runningSuiteMatch = trimmed.match(RUNNING_SUITE_PATTERN);
    if (runningSuiteMatch) {
      currentSuite = getOrCreateSuite(suites, runningSuiteMatch[1] ?? "workspace");
      continue;
    }

    const docTestSuiteMatch = trimmed.match(DOC_TEST_SUITE_PATTERN);
    if (docTestSuiteMatch) {
      currentSuite = getOrCreateSuite(
        suites,
        `Doc-tests ${docTestSuiteMatch[1] ?? ""}`.trim(),
      );
      continue;
    }

    const testResultMatch = trimmed.match(TEST_RESULT_PATTERN);
    if (!testResultMatch) {
      continue;
    }

    const name = testResultMatch[1] ?? trimmed;
    const rawStatus = testResultMatch[2] ?? "FAILED";
    const status =
      rawStatus === "ok"
        ? "passed"
        : rawStatus === "ignored"
          ? "ignored"
          : "failed";
    const testCase: RustTestCase = {
      name,
      status,
      output: failureBlocks.get(name),
    };

    currentSuite.tests.push(testCase);
  }

  const nonEmptySuites = suites.filter((suite) => suite.tests.length > 0);
  const allTests = nonEmptySuites.flatMap((suite) => suite.tests);

  return {
    suites: nonEmptySuites,
    summary: {
      passed: allTests.filter((testCase) => testCase.status === "passed").length,
      failed: allTests.filter((testCase) => testCase.status === "failed").length,
      ignored: allTests.filter((testCase) => testCase.status === "ignored").length,
      total: allTests.length,
    },
  };
}

export function parseRustCoverageSummary(
  rawJson: string,
  cwd: string,
): RustCoverageSummary {
  const payload = JSON.parse(rawJson) as LlvmCovJsonLike;
  const dataSets = payload.data ?? [];
  const totalsAccumulator = createMetricAccumulator();
  const groups = new Map<string, ReturnType<typeof createMetricAccumulator>>();

  for (const data of dataSets) {
    for (const key of COVERAGE_KEYS) {
      const metric = data.totals?.[key];
      if (metric?.count != null && metric.covered != null) {
        totalsAccumulator[key].count += metric.count;
        totalsAccumulator[key].covered += metric.covered;
      }
    }

    for (const file of data.files ?? []) {
      if (!file.filename) {
        continue;
      }

      const groupName = buildCoverageGroupName(cwd, normalizePath(file.filename));
      const accumulator = groups.get(groupName) ?? createMetricAccumulator();

      for (const key of COVERAGE_KEYS) {
        const metric = file.summary?.[key];
        if (metric?.count != null && metric.covered != null) {
          accumulator[key].count += metric.count;
          accumulator[key].covered += metric.covered;
        }
      }

      groups.set(groupName, accumulator);
    }
  }

  const totals = materializeMetricRecord(totalsAccumulator);
  const groupSummaries: RustCoverageGroup[] = [...groups.entries()]
    .map(([name, accumulator]) => ({
      name,
      metrics: materializeMetricRecord(accumulator),
    }))
    .filter((group) => Object.keys(group.metrics).length > 0)
    .toSorted((left, right) => left.name.localeCompare(right.name));

  return {
    totals,
    groups: groupSummaries,
  };
}

function extractFailureBlocks(lines: string[]): Map<string, string> {
  const blocks = new Map<string, string>();

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]?.trim();
    const match = line?.match(FAILURE_BLOCK_PATTERN);
    if (!match) {
      continue;
    }

    const testName = match[1]?.trim();
    const stream = match[2];
    const bodyLines: string[] = [];
    let pointer = index + 1;

    while (pointer < lines.length) {
      const nextLine = lines[pointer] ?? "";
      const trimmedNextLine = nextLine.trim();

      if (
        FAILURE_BLOCK_PATTERN.test(trimmedNextLine) ||
        trimmedNextLine === "failures:" ||
        trimmedNextLine.startsWith("test result:")
      ) {
        break;
      }

      bodyLines.push(nextLine);
      pointer += 1;
    }

    index = pointer - 1;

    if (!testName) {
      continue;
    }

    const current = blocks.get(testName);
    const body = bodyLines.join("\n").trim();
    const section = body ? `[${stream}]\n${body}` : `[${stream}]`;
    blocks.set(testName, current ? `${current}\n\n${section}` : section);
  }

  return blocks;
}

function getOrCreateSuite(suites: RustSuite[], name: string): RustSuite {
  const existing = suites.find((suite) => suite.name === name);
  if (existing) {
    return existing;
  }

  const suite: RustSuite = {
    name,
    tests: [],
  };
  suites.push(suite);
  return suite;
}

function createMetricAccumulator(): Record<
  RustCoverageMetricKey,
  { covered: number; count: number }
> {
  return {
    lines: { covered: 0, count: 0 },
    functions: { covered: 0, count: 0 },
    regions: { covered: 0, count: 0 },
    branches: { covered: 0, count: 0 },
    instantiations: { covered: 0, count: 0 },
  };
}

function materializeMetricRecord(
  accumulator: Record<RustCoverageMetricKey, { covered: number; count: number }>,
): Partial<Record<RustCoverageMetricKey, CoverageMetric>> {
  const metrics: Partial<Record<RustCoverageMetricKey, CoverageMetric>> = {};

  for (const key of COVERAGE_KEYS) {
    if (accumulator[key].count <= 0) {
      continue;
    }

    metrics[key] = createCoverageMetric(
      accumulator[key].covered,
      accumulator[key].count,
    );
  }

  return metrics;
}
