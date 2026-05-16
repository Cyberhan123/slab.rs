import { describe, expect, it } from 'vitest';

import {
  extractTaskId,
  isFailedTaskStatus,
  isSettledStatus,
  normalizeTaskProgress,
} from '../utils';

describe('task utils', () => {
  it('extracts operation ids from accepted task responses', () => {
    expect(extractTaskId({ operation_id: ' task-1 ' })).toBe('task-1');
    expect(extractTaskId({ task_id: 'task-2' })).toBe('task-2');
  });

  it('ignores missing or blank task ids', () => {
    expect(extractTaskId(null)).toBeNull();
    expect(extractTaskId({ operation_id: '   ' })).toBeNull();
    expect(extractTaskId({ task_id: 42 })).toBeNull();
  });

  it('classifies settled and failed terminal statuses', () => {
    expect(isSettledStatus('succeeded')).toBe(true);
    expect(isSettledStatus('running')).toBe(false);
    expect(isFailedTaskStatus('failed')).toBe(true);
    expect(isFailedTaskStatus('cancelled')).toBe(true);
    expect(isFailedTaskStatus('interrupted')).toBe(true);
    expect(isFailedTaskStatus('succeeded')).toBe(false);
  });

  it('normalizes task progress payloads', () => {
    expect(
      normalizeTaskProgress({
        label: 'Downloading',
        current: 25,
        total: 50,
        step: 2,
        step_count: 4,
        unit: 'bytes',
      }),
    ).toEqual({
      label: 'Downloading',
      current: 25,
      total: 50,
      step: 2,
      stepCount: 4,
      unit: 'bytes',
    });
    expect(normalizeTaskProgress({ current: Number.NaN })).toBeNull();
  });
});
