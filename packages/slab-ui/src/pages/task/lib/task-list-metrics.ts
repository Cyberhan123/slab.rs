import { clamp, countBy, sortBy, sumBy } from 'lodash-es';

import { PAGE_SIZE, type Task } from '../const';
import { getSparklineWeight, getTaskDurationMs, isSettledStatus } from '../utils';

export type TaskMetrics = {
  total: number;
  running: number;
  queued: number;
  failed: number;
  succeeded: number;
};

export const SUCCESS_SPARKLINE_FALLBACK = [0.32, 0.48, 0.44, 0.66, 0.82, 0.72, 0.77];
export const DURATION_SPARKLINE_FALLBACK = [0.18, 0.28, 0.24, 0.6, 0.44];

/** Counts tasks by lifecycle status. Queued tracks the backend "pending" bucket. */
export function computeTaskMetrics(allTasks: Task[]): TaskMetrics {
  const byStatus = countBy(allTasks, 'status');
  return {
    total: allTasks.length,
    running: byStatus.running ?? 0,
    queued: byStatus.pending ?? 0,
    failed: byStatus.failed ?? 0,
    succeeded: byStatus.succeeded ?? 0,
  };
}

export function selectSettledTasks(allTasks: Task[]): Task[] {
  return allTasks.filter((task) => isSettledStatus(task.status));
}

export function computeSuccessRate(settledTaskCount: number, succeeded: number): number {
  if (settledTaskCount === 0) return 0;
  return (succeeded / settledTaskCount) * 100;
}

export function computeAverageTurnaroundMs(settledTasks: Task[]): number {
  if (settledTasks.length === 0) return 0;
  const totalDuration = sumBy(settledTasks, getTaskDurationMs);
  return totalDuration / settledTasks.length;
}

export function computeSuccessSparkline(allTasks: Task[]): number[] {
  const recentTasks = sortBy(allTasks, (task) => Date.parse(task.updated_at)).slice(-7);

  if (recentTasks.length === 0) {
    return SUCCESS_SPARKLINE_FALLBACK;
  }

  return recentTasks.map((task) => getSparklineWeight(task.status));
}

export function computeDurationSparkline(settledTasks: Task[]): number[] {
  const samples = sortBy(settledTasks, (task) => Date.parse(task.updated_at))
    .slice(-5)
    .map((task) => getTaskDurationMs(task));

  if (samples.length === 0) {
    return DURATION_SPARKLINE_FALLBACK;
  }

  const maxSample = Math.max(...samples, 1);

  return samples.map((sample) => clamp(sample / maxSample, 0.16, Number.POSITIVE_INFINITY));
}

export function computeTotalPages(taskCount: number): number {
  return Math.max(1, Math.ceil(taskCount / PAGE_SIZE));
}

export function paginateTasks(allTasks: Task[], currentPage: number): Task[] {
  const startIndex = (currentPage - 1) * PAGE_SIZE;
  return allTasks.slice(startIndex, startIndex + PAGE_SIZE);
}

export type PaginationRange = { start: number; end: number };

/** Returns null when there are no tasks (the empty-state label is i18n-only). */
export function computePaginationRange(taskCount: number, currentPage: number): PaginationRange | null {
  if (taskCount === 0) {
    return null;
  }

  const start = (currentPage - 1) * PAGE_SIZE + 1;
  const end = clamp(currentPage * PAGE_SIZE, 0, taskCount);

  return { start, end };
}
