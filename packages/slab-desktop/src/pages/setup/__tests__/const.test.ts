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
      message: null,
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
    expect(getProvisionStageLabel('running', null, false)).toBe('Preparing Slab runtime');
    expect(getProvisionStageLabel('running', null, true)).toBe('Verifying installed runtime');
    expect(getProvisionStageLabel('failed', null)).toBe('Setup failed');
    expect(getProvisionStageLabel('succeeded', null)).toBe('Setup finished');
    expect(getProvisionStageLabel('idle', null, false)).toBe('Preparing environment');
    expect(getProvisionStageLabel('idle', null, true)).toBe('Checking desktop environment');

    expect(getProvisionStageHint('failed', null, false)).toBe(
      'Review the error below, then retry the setup task.',
    );
    expect(getProvisionStageHint('failed', null, true)).toBe(
      'Review the error below, then retry the local prerequisite check.',
    );
    expect(getProvisionStageHint('succeeded', null, false)).toBe(
      'Runtime payloads are in place. Launching Slab now.',
    );
    expect(getProvisionStageHint('succeeded', null, true)).toBe(
      'FFmpeg and local runtime checks are complete. Launching Slab now.',
    );
    expect(getProvisionStageHint('starting', null, false)).toBe(
      'Creating the setup task and connecting to the local host.',
    );
    expect(getProvisionStageHint('starting', null, true)).toBe(
      'Inspecting the installed runtime and checking whether FFmpeg is already available.',
    );
    expect(getProvisionStageHint('running', null, false)).toBe(
      'Downloading payloads, verifying CABs, checking FFmpeg, and restarting runtime workers.',
    );
    expect(getProvisionStageHint('running', null, true)).toBe(
      'Checking FFmpeg runtime availability and confirming local workers are ready.',
    );
    expect(getProvisionStageHint('idle', null, false)).toBe(
      'Inspecting the local desktop installation.',
    );
    expect(getProvisionStageHint('idle', null, true)).toBe(
      'Inspecting the local desktop installation and FFmpeg availability.',
    );
  });

  it('summarizes explicit setup step counts in stage hints', () => {
    expect(
      getProvisionStageHint(
        'running',
        createTask({
          current: 0,
          step: 2,
          step_count: 5,
        }),
      ),
    ).toBe('Step 2 of 5');
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
    expect(getProvisionProgressValue('running', null)).toBe(MIN_ACTIVE_PROGRESS);
    expect(getProvisionProgressValue('idle', null)).toBe(0);
    expect(getProvisionProgressValue('succeeded', null)).toBe(100);
    expect(
      getProvisionProgressValue(
        'running',
        createTask({
          current: 2,
          step: 1,
          step_count: 3,
        }),
      ),
    ).toBe(0);
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
    expect(getProvisionProgressSummary('failed', null, false)).toBe(
      'Provisioning stopped before setup could complete.',
    );
    expect(getProvisionProgressSummary('succeeded', null)).toBe('100% complete');
    expect(
      getProvisionProgressSummary(
        'running',
        createTask({
          current: 0,
          step: 3,
          step_count: 5,
        }),
      ),
    ).toBe('Stage 3/5');
    expect(getProvisionProgressSummary('starting', null, false)).toBe('Creating setup task...');
    expect(getProvisionProgressSummary('starting', null, true)).toBe('Checking installed runtime...');
    expect(getProvisionProgressSummary('running', null, false)).toBe('Waiting for progress updates...');
    expect(getProvisionProgressSummary('running', null, true)).toBe('Checking FFmpeg and local workers...');
    expect(getProvisionProgressSummary('idle', null)).toBe('Waiting to begin');
  });
});
