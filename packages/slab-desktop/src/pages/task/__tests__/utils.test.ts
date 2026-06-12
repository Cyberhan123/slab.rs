import { describe, expect, it } from 'vitest';

import {
  extractTaskId,
  formatCompactDuration,
  formatPercent,
  formatTaskId,
  getSparklineWeight,
  getStatusTone,
  getTaskDeepLink,
  getTaskDurationMs,
  getTaskTypeMeta,
  isFailedTaskStatus,
  isMediaTaskType,
  isSettledStatus,
  normalizeTaskProgress,
} from '../utils';

const translate = (key: string) => key;
const serverTranslate = (key: string, options?: Record<string, unknown>) => {
  if (key === 'server.tasks.setup.downloadingPayload') {
    return `translated ${options?.file_name}`;
  }

  return typeof options?.defaultValue === 'string' ? options.defaultValue : key;
};

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

  it('falls back to task_id when operation_id is unusable', () => {
    expect(extractTaskId({ operation_id: ' ', task_id: 'task-2' })).toBe('task-2');
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
      message: null,
      current: 25,
      total: 50,
      step: 2,
      stepCount: 4,
      unit: 'bytes',
    });
    expect(normalizeTaskProgress({ current: Number.NaN })).toBeNull();
  });

  it('translates task progress fields and falls back to legacy strings', () => {
    expect(
      normalizeTaskProgress(
        {
          label: 'Downloading payload',
          message: 'Legacy message',
          i18n: {
            label: {
              key: 'server.tasks.setup.downloadingPayload',
              params: { file_name: 'runtime.cab' },
            },
            message: {
              key: 'server.tasks.setup.checkingFfmpeg',
            },
          },
          current: 1,
        },
        serverTranslate,
      ),
    ).toMatchObject({
      label: 'translated runtime.cab',
      message: 'Legacy message',
    });
  });

  it('formats task values defensively', () => {
    expect(formatPercent(Number.NaN)).toBe('0.0%');
    expect(formatPercent(12.345)).toBe('12.3%');
    expect(formatCompactDuration(0)).toBe('<1s');
    expect(formatCompactDuration(9_500)).toBe('9.5s');
    expect(formatCompactDuration(65_000)).toBe('1.1m');
    expect(formatCompactDuration(3 * 60 * 60 * 1000)).toBe('3.0h');
    expect(formatTaskId('123e4567-e89b-12d3-a456-426614174000')).toBe('#123E4567');
    expect(
      getTaskDurationMs({
        created_at: '2026-01-01T00:00:10Z',
        updated_at: '2026-01-01T00:00:00Z',
      }),
    ).toBe(0);
    expect(getTaskDurationMs({ created_at: 'bad', updated_at: 'also bad' })).toBe(0);
  });

  it('maps status, task types, and media deep links', () => {
    expect(getSparklineWeight('succeeded')).toBe(0.92);
    expect(getSparklineWeight('unknown')).toBe(0.48);
    expect(getStatusTone('failed', translate)).toMatchObject({
      label: 'pages.task.status.failed',
      dotClassName: 'bg-destructive',
    });
    expect(getStatusTone('custom', translate)).toMatchObject({ label: 'custom' });

    expect(getTaskTypeMeta('stable_diffusion', translate)).toMatchObject({
      label: 'pages.task.taskType.imageGeneration',
    });
    expect(getTaskTypeMeta('model.download', translate)).toMatchObject({
      label: 'pages.task.taskType.modelDownload',
    });
    expect(getTaskTypeMeta('custom.task_type', translate)).toMatchObject({
      label: 'Custom Task Type',
    });

    expect(isMediaTaskType('image_generation')).toBe(true);
    expect(isMediaTaskType('model_download')).toBe(false);
    expect(getTaskDeepLink('video_generation', 'task-1')).toBe('/video?task=task-1');
    expect(getTaskDeepLink('model_download', 'task-1')).toBeNull();
  });
});
