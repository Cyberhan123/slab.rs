import { describe, expect, expectTypeOf, it } from 'vitest';

import type { Task } from '../../const';
import {
  computeAverageTurnaroundMs,
  computeDurationSparkline,
  computePaginationRange,
  computeSuccessRate,
  computeSuccessSparkline,
  computeTaskMetrics,
  computeTotalPages,
  DURATION_SPARKLINE_FALLBACK,
  paginateTasks,
  type PaginationRange,
  type TaskMetrics,
  selectSettledTasks,
  SUCCESS_SPARKLINE_FALLBACK,
} from '../task-list-metrics';

describe('task metrics return types (expectTypeOf)', () => {
  it('computeTaskMetrics returns TaskMetrics', () => {
    expectTypeOf(computeTaskMetrics).returns.toEqualTypeOf<TaskMetrics>();
  });

  it('computePaginationRange returns PaginationRange | null', () => {
    expectTypeOf(computePaginationRange).returns.toEqualTypeOf<PaginationRange | null>();
  });
});

function task(overrides: { status: string } & Record<string, unknown>): Task {
  return {
    id: 'task-1',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:30Z',
    ...overrides,
  } as unknown as Task;
}

describe('task list metrics', () => {
  it('counts tasks by lifecycle status, mapping pending to queued', () => {
    const metrics = computeTaskMetrics([
      task({ status: 'running' }),
      task({ status: 'pending' }),
      task({ status: 'succeeded' }),
      task({ status: 'failed' }),
    ]);

    expect(metrics).toEqual({ total: 4, running: 1, queued: 1, failed: 1, succeeded: 1 });
  });

  it('selects only settled tasks', () => {
    const settled = selectSettledTasks([
      task({ status: 'running' }),
      task({ status: 'succeeded' }),
      task({ status: 'failed' }),
      task({ status: 'cancelled' }),
      task({ status: 'interrupted' }),
      task({ status: 'pending' }),
    ]);

    expect(settled.map((item) => item.status)).toEqual([
      'succeeded',
      'failed',
      'cancelled',
      'interrupted',
    ]);
  });

  it('computes success rate over settled tasks and zeroes the empty case', () => {
    expect(computeSuccessRate(0, 0)).toBe(0);
    expect(computeSuccessRate(4, 3)).toBe(75);
  });

  it('averages turnaround duration across settled tasks', () => {
    const settled = [
      task({
        status: 'succeeded',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:01:00Z',
      }),
      task({
        status: 'succeeded',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:02:00Z',
      }),
    ];

    expect(computeAverageTurnaroundMs(settled)).toBe(90_000);
    expect(computeAverageTurnaroundMs([])).toBe(0);
  });

  it('returns the success sparkline fallback when there are no tasks', () => {
    expect(computeSuccessSparkline([])).toEqual(SUCCESS_SPARKLINE_FALLBACK);
  });

  it('maps the most recent tasks to sparkline weights by recency', () => {
    const sparkline = computeSuccessSparkline([
      task({ status: 'failed', updated_at: '2026-01-01T00:00:00Z' }),
      task({ status: 'succeeded', updated_at: '2026-01-02T00:00:00Z' }),
    ]);

    expect(sparkline).toEqual([0.4, 0.92]);
  });

  it('returns the duration sparkline fallback when there are no settled tasks', () => {
    expect(computeDurationSparkline([])).toEqual(DURATION_SPARKLINE_FALLBACK);
  });

  it('normalizes the most recent settled durations by the max sample', () => {
    const settled = [
      task({
        status: 'succeeded',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:30Z',
      }),
      task({
        status: 'succeeded',
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:01:00Z',
      }),
    ];

    expect(computeDurationSparkline(settled)).toEqual([0.5, 1]);
  });

  it('computes total pages with a minimum of one', () => {
    expect(computeTotalPages(0)).toBe(1);
    expect(computeTotalPages(4)).toBe(1);
    expect(computeTotalPages(5)).toBe(2);
    expect(computeTotalPages(9)).toBe(3);
  });

  it('paginates the task list by page size', () => {
    const tasks = [
      task({ id: 't1', status: 'succeeded' }),
      task({ id: 't2', status: 'succeeded' }),
      task({ id: 't3', status: 'succeeded' }),
      task({ id: 't4', status: 'succeeded' }),
      task({ id: 't5', status: 'succeeded' }),
    ];

    expect(paginateTasks(tasks, 1).map((item) => item.id)).toEqual(['t1', 't2', 't3', 't4']);
    expect(paginateTasks(tasks, 2).map((item) => item.id)).toEqual(['t5']);
  });

  it('builds a clamped pagination range and reports null when empty', () => {
    expect(computePaginationRange(0, 1)).toBeNull();
    expect(computePaginationRange(1, 1)).toEqual({ start: 1, end: 1 });
    // PAGE_SIZE is 4: page 2 starts at index 4, end clamps to the task count.
    expect(computePaginationRange(5, 2)).toEqual({ start: 5, end: 5 });
  });
});
