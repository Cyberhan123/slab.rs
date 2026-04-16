import { ApiError, apiClient } from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';

export const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
export const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

export type ModelDownloadProgress = {
  label: string | null;
  current: number;
  total: number | null;
  unit: string | null;
  step: number | null;
  step_count: number | null;
};

export type DownloadTrackingState = {
  taskId: string;
  progress: ModelDownloadProgress | null;
};

type TaskResponse = components['schemas']['TaskResponse'];

export const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export async function startModelDownloadTask(modelId: string) {
  const { data, error, response } = await apiClient.POST('/v1/models/download', {
    body: {
      model_id: modelId,
    },
  });

  if (!response.ok || error) {
    throw ApiError.fromResponse(response, error);
  }

  if (!data) {
    throw new Error('Model download request returned an empty response.');
  }

  return data;
}

export async function getModelDownloadTask(taskId: string): Promise<TaskResponse> {
  const { data, error, response } = await apiClient.GET('/v1/tasks/{id}', {
    params: {
      path: { id: taskId },
    },
  });

  if (!response.ok || error) {
    throw ApiError.fromResponse(response, error);
  }

  if (!data) {
    throw new Error(`Task '${taskId}' returned an empty response.`);
  }

  return data;
}

export function extractTaskId(payload: unknown): string | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }

  const taskId =
    (payload as { operation_id?: unknown }).operation_id ??
    (payload as { task_id?: unknown }).task_id;

  if (typeof taskId !== 'string') {
    return null;
  }

  const trimmed = taskId.trim();
  return trimmed.length > 0 ? trimmed : null;
}

export function normalizeTaskProgress(value: unknown): ModelDownloadProgress | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }

  const progress = value as Record<string, unknown>;
  if (typeof progress.current !== 'number' || !Number.isFinite(progress.current)) {
    return null;
  }

  const total =
    typeof progress.total === 'number' && Number.isFinite(progress.total) ? progress.total : null;
  const step =
    typeof progress.step === 'number' && Number.isFinite(progress.step) ? progress.step : null;
  const stepCount =
    typeof progress.step_count === 'number' && Number.isFinite(progress.step_count)
      ? progress.step_count
      : null;

  return {
    label: typeof progress.label === 'string' ? progress.label : null,
    current: progress.current,
    total,
    unit: typeof progress.unit === 'string' ? progress.unit : null,
    step,
    step_count: stepCount,
  };
}
