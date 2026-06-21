import { ApiError, apiClient } from '@slab/api';
import { SERVER_BASE_URL } from '@slab/api/config';
import type { components } from '@slab/api/v1';

export type MediaTaskStatus = components['schemas']['TaskStatus'];
export type MediaTaskProgress = components['schemas']['TaskProgressResponse'] | null;
export type ImageGenerationTask = components['schemas']['ImageGenerationTaskResponse'];
export type VideoGenerationTask = components['schemas']['VideoGenerationTaskResponse'];
export type AudioTranscriptionTask = components['schemas']['AudioTranscriptionTaskResponse'];
export type GenerationProgressStage = 'queued' | 'running' | 'finalizing';
export type GenerationProgress = {
  percent: number | null;
  stage: GenerationProgressStage;
  etaMs: number | null;
  stepLabel: string | null;
  message: string | null;
  current: number;
  total: number | null;
  updatedAt: number;
};

function buildApiUrl(path: string): string {
  return new URL(path.replace(/^\//, ''), `${SERVER_BASE_URL}/`).toString();
}

export function resolveMediaUrl(path?: string | null): string | null {
  if (!path) {
    return null;
  }

  if (/^https?:\/\//i.test(path)) {
    return path;
  }

  return buildApiUrl(path);
}

export function deriveProgress(
  progress: MediaTaskProgress,
  previous?: GenerationProgress | null,
  now = Date.now(),
): GenerationProgress {
  if (!progress) {
    return {
      current: 0,
      etaMs: null,
      message: null,
      percent: null,
      stage: 'queued',
      stepLabel: null,
      total: null,
      updatedAt: now,
    };
  }

  const current = finiteNumber(progress.current) ?? 0;
  const total = finiteNumber(progress.total);
  const percent = total && total > 0 ? clampPercent((current / total) * 100) : null;
  const label = progress.label?.trim() || null;
  const message = progress.message?.trim() || null;
  const step = finiteNumber(progress.step);
  const stepCount = finiteNumber(progress.step_count);

  return {
    current,
    etaMs: estimateEtaMs(current, total, previous, now),
    message,
    percent,
    stage: percent !== null && percent >= 99 ? 'finalizing' : 'running',
    stepLabel: buildStepLabel(label, step, stepCount),
    total,
    updatedAt: now,
  };
}

function requireApiData<T>(
  result: { data?: T; error?: unknown; response: Response },
  emptyMessage: string,
): T {
  if (!result.response.ok || result.error) {
    throw ApiError.fromResponse(result.response, result.error);
  }

  if (result.data === undefined) {
    throw new Error(emptyMessage);
  }

  return result.data;
}

export async function listImageGenerations(): Promise<ImageGenerationTask[]> {
  return requireApiData(
    await apiClient.GET('/v1/images/generations'),
    'Image generation history returned an empty response.',
  );
}

export async function getImageGeneration(taskId: string): Promise<ImageGenerationTask> {
  return requireApiData(
    await apiClient.GET('/v1/images/generations/{id}', {
      params: { path: { id: taskId } },
    }),
    `Image generation '${taskId}' returned an empty response.`,
  );
}

export async function listVideoGenerations(): Promise<VideoGenerationTask[]> {
  return requireApiData(
    await apiClient.GET('/v1/video/generations'),
    'Video generation history returned an empty response.',
  );
}

export async function getVideoGeneration(taskId: string): Promise<VideoGenerationTask> {
  return requireApiData(
    await apiClient.GET('/v1/video/generations/{id}', {
      params: { path: { id: taskId } },
    }),
    `Video generation '${taskId}' returned an empty response.`,
  );
}

export async function listAudioTranscriptions(): Promise<AudioTranscriptionTask[]> {
  return requireApiData(
    await apiClient.GET('/v1/audio/transcriptions'),
    'Audio transcription history returned an empty response.',
  );
}

export async function getAudioTranscription(taskId: string): Promise<AudioTranscriptionTask> {
  return requireApiData(
    await apiClient.GET('/v1/audio/transcriptions/{id}', {
      params: { path: { id: taskId } },
    }),
    `Audio transcription '${taskId}' returned an empty response.`,
  );
}

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function clampPercent(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.min(Math.max(value, 0), 100);
}

function estimateEtaMs(
  current: number,
  total: number | null,
  previous: GenerationProgress | null | undefined,
  now: number,
): number | null {
  if (!total || total <= current || !previous || previous.current >= current) {
    return null;
  }

  const elapsedMs = now - previous.updatedAt;
  const delta = current - previous.current;
  if (elapsedMs <= 0 || delta <= 0) {
    return null;
  }

  return Math.max(Math.round(((total - current) / delta) * elapsedMs), 0);
}

function buildStepLabel(
  label: string | null,
  step: number | null,
  stepCount: number | null,
): string | null {
  if (label && step && stepCount) {
    return `${label} (${step}/${stepCount})`;
  }

  if (label) {
    return label;
  }

  if (step && stepCount) {
    return `Step ${step}/${stepCount}`;
  }

  return null;
}
