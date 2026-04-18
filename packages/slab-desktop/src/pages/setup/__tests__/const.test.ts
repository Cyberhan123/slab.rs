import { describe, expect, it } from 'vitest';

import type { TaskProgress, TaskRecord } from '../const';
import {
  MIN_ACTIVE_PROGRESS,
  getProvisionProgressSummary,
  getProvisionProgressValue,
  getProvisionStageHint,
  getProvisionStageLabel,
  normalizeTaskProgress,
} from '../const';

function createTask(progress?: Partial<TaskProgress>): TaskRecord {
  return {
    progress: progress as TaskProgress,
  } as TaskRecord;
}

describe('setup progress helpers', () => {
  it('normalizes valid progress payloads and preserves typed values', () => {
    expect(
      normalizeTaskProgress({
        label: '  Downloading runtime payloads  ',
        current: 25,
        total: 50,
        step: 2,
        step_count: 4,
        unit: 'files',
      } as TaskProgress),
    ).toEqual({
      label: '  Downloading runtime payloads  ',
      current: 25,
      total: 50,
      step: 2,
      stepCount: 4,
      unit: 'files',
    });
  });

  it('returns null for invalid progress payloads', () => {
    expect(normalizeTaskProgress(undefined)).toBeNull();
    expect(
      normalizeTaskProgress({
        label: 'Bad payload',
        current: Number.NaN,
      } as TaskProgress),
    ).toBeNull();
  });

  it('prefers explicit progress labels when present', () => {
    const task = createTask({
      label: '  Verifying FFmpeg  ',
      current: 1,
      total: 4,
    });

    expect(getProvisionStageLabel('running', task)).toBe('Verifying FFmpeg');
  });

  it('derives stage labels and hints from the runtime payload mode', () => {
    expect(getProvisionStageLabel('starting', null, false)).toBe('Starting setup');
    expect(getProvisionStageLabel('starting', null, true)).toBe('Checking desktop prerequisites');

    expect(getProvisionStageHint('running', null, false)).toBe(
      'Downloading payloads, verifying CABs, checking FFmpeg, and restarting runtime workers.',
    );
    expect(getProvisionStageHint('running', null, true)).toBe(
      'Checking FFmpeg, downloading it when needed, and confirming local workers are ready.',
    );
  });

  it('calculates stepped progress and caps unfinished work below 100%', () => {
    const steppedTask = createTask({
      current: 25,
      total: 50,
      step: 2,
      step_count: 4,
    });

    expect(getProvisionProgressValue('running', steppedTask)).toBe(37.5);
    expect(
      getProvisionProgressValue(
        'running',
        createTask({
          current: 999,
          total: 1,
        }),
      ),
    ).toBe(99);
    expect(getProvisionProgressValue('starting', null)).toBe(MIN_ACTIVE_PROGRESS);
    expect(getProvisionProgressValue('succeeded', null)).toBe(100);
  });

  it('summarizes progress for active, failed, and finished states', () => {
    expect(
      getProvisionProgressSummary(
        'running',
        createTask({
          current: 25,
          total: 50,
        }),
      ),
    ).toBe('50% complete');
    expect(getProvisionProgressSummary('failed', null, true)).toBe(
      'Desktop prerequisite checks stopped before setup could complete.',
    );
    expect(getProvisionProgressSummary('succeeded', null)).toBe('100% complete');
  });
});
