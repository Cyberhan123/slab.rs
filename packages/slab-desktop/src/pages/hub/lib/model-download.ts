import { ApiError, apiClient } from '@slab/api';
import type { components } from '@slab/api/v1';
import type { NormalizedTaskProgress } from '@/pages/task/utils';

export {
  extractTaskId,
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  normalizeTaskProgress,
  sleep,
} from '@/pages/task/utils';

export type ModelDownloadProgress = NormalizedTaskProgress;

export type DownloadTrackingState = {
  taskId: string;
  progress: ModelDownloadProgress | null;
};

type TaskResponse = components['schemas']['TaskResponse'];

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
