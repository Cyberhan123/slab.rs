import { describe, expect, it } from 'vitest';

import { deriveProgress } from '../media-task-api';

describe('deriveProgress', () => {
  it('returns queued state when no backend progress exists yet', () => {
    expect(deriveProgress(null, null, 1000)).toEqual({
      current: 0,
      etaMs: null,
      message: null,
      percent: null,
      stage: 'queued',
      stepLabel: null,
      total: null,
      updatedAt: 1000,
    });
  });

  it('projects percent, step labels, and ETA from current and previous samples', () => {
    const previous = deriveProgress(
      { current: 10, label: 'Sampling', message: null, step: 1, step_count: 2, total: 100 },
      null,
      1000,
    );

    expect(
      deriveProgress(
        { current: 30, label: 'Sampling', message: 'denoising', step: 1, step_count: 2, total: 100 },
        previous,
        3000,
      ),
    ).toEqual({
      current: 30,
      etaMs: 7000,
      message: 'denoising',
      percent: 30,
      stage: 'running',
      stepLabel: 'Sampling (1/2)',
      total: 100,
      updatedAt: 3000,
    });
  });

  it('marks nearly complete progress as finalizing and clamps impossible percentages', () => {
    expect(
      deriveProgress({ current: 120, label: 'Writing', total: 100 }, null, 1000),
    ).toMatchObject({
      percent: 100,
      stage: 'finalizing',
      stepLabel: 'Writing',
    });
  });
});
