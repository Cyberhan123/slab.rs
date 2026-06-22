import { Download, Film, Image, ListChecks, Mic } from 'lucide-react';
import { translateServerField, type ServerI18nPayload } from '@slab/i18n';

type Translate = (key: string, options?: Record<string, unknown>) => string;

export interface NormalizedTaskProgress {
  label: string | null;
  message: string | null;
  current: number;
  total: number | null;
  step: number | null;
  stepCount: number | null;
  unit: string | null;
}

export function formatDateTime(value: string, locale: string) {
  return new Date(value).toLocaleString(locale, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  });
}

export function formatPercent(value: number) {
  if (!Number.isFinite(value)) {
    return '0.0%';
  }

  return `${value.toFixed(1)}%`;
}

export function formatCompactDuration(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return '<1s';
  }

  const seconds = value / 1000;
  if (seconds < 60) {
    return `${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
  }

  const minutes = seconds / 60;
  if (minutes < 60) {
    return `${minutes < 10 ? minutes.toFixed(1) : Math.round(minutes)}m`;
  }

  const hours = minutes / 60;
  return `${hours < 10 ? hours.toFixed(1) : Math.round(hours)}h`;
}

export function formatTaskId(value: string) {
  return `#${value.replace(/-/g, '').slice(0, 8).toUpperCase()}`;
}

export function getTaskDurationMs(task: { created_at: string; updated_at: string }) {
  const createdAt = Date.parse(task.created_at);
  const updatedAt = Date.parse(task.updated_at);

  if (Number.isNaN(createdAt) || Number.isNaN(updatedAt)) {
    return 0;
  }

  return Math.max(updatedAt - createdAt, 0);
}

const SETTLED_TASK_STATUSES = ['succeeded', 'failed', 'cancelled', 'interrupted'] as const;
const FAILED_TASK_STATUSES = ['failed', 'cancelled', 'interrupted'] as const;
export const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
export const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

/**
 * Task type that owns a restartable lifecycle on the backend. The server rejects
 * restart for any other task type with a 400 ("does not support restart"), so the
 * UI must only offer Restart for this type — otherwise every failed media task
 * surfaces a button that deterministically fails when clicked.
 */
export const MODEL_DOWNLOAD_TASK_TYPE = 'model_download';

export function canRestartTaskType(taskType: string) {
  return taskType === MODEL_DOWNLOAD_TASK_TYPE;
}

export function isSettledStatus(status: string) {
  return SETTLED_TASK_STATUSES.includes(status as (typeof SETTLED_TASK_STATUSES)[number]);
}

export function isFailedTaskStatus(status: string) {
  return FAILED_TASK_STATUSES.includes(status as (typeof FAILED_TASK_STATUSES)[number]);
}

export function extractTaskId(payload: unknown): string | null {
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }

  for (const taskId of [
    (payload as { operation_id?: unknown }).operation_id,
    (payload as { task_id?: unknown }).task_id,
  ]) {
    if (typeof taskId !== 'string') {
      continue;
    }

    const trimmed = taskId.trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
  }

  return null;
}

export const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

export function normalizeTaskProgress(
  progress: unknown,
  t?: Translate,
): NormalizedTaskProgress | null {
  if (
    typeof progress !== 'object' ||
    progress === null ||
    typeof (progress as { current?: unknown }).current !== 'number' ||
    !Number.isFinite((progress as { current: number }).current)
  ) {
    return null;
  }

  const value = progress as Record<string, unknown>;
  const i18n = value.i18n as ServerI18nPayload;
  const label = typeof value.label === 'string' ? value.label : null;
  const message = typeof value.message === 'string' ? value.message : null;

  return {
    label: t ? translateServerField(i18n, 'label', label, t) || null : label,
    message: t ? translateServerField(i18n, 'message', message, t) || null : message,
    current: value.current as number,
    total: finiteNumberOrNull(value.total),
    step: finiteNumberOrNull(value.step),
    stepCount: finiteNumberOrNull(value.step_count),
    unit: typeof value.unit === 'string' ? value.unit : null,
  };
}

function finiteNumberOrNull(value: unknown) {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

export function getSparklineWeight(status: string) {
  switch (status) {
    case 'succeeded':
      return 0.92;
    case 'running':
      return 0.72;
    case 'pending':
      return 0.58;
    case 'failed':
      return 0.4;
    case 'cancelled':
    case 'interrupted':
      return 0.3;
    default:
      return 0.48;
  }
}

export function getStatusTone(status: string, t: Translate) {
  switch (status) {
    case 'succeeded':
      return {
        label: t('pages.task.status.succeeded'),
        className: 'bg-[var(--status-success-bg)] text-[color:var(--success)]',
        dotClassName: 'bg-[var(--success)]',
      };
    case 'running':
      return {
        label: t('pages.task.status.running'),
        className: 'bg-[var(--status-info-bg)] text-primary',
        dotClassName: 'bg-primary',
      };
    case 'failed':
      return {
        label: t('pages.task.status.failed'),
        className: 'bg-[var(--status-danger-bg)] text-destructive',
        dotClassName: 'bg-destructive',
      };
    case 'pending':
      return {
        label: t('pages.task.status.pending'),
        className: 'bg-[var(--status-neutral-bg)] text-muted-foreground',
        dotClassName: 'bg-muted-foreground',
      };
    case 'cancelled':
      return {
        label: t('pages.task.status.cancelled'),
        className: 'bg-[var(--status-neutral-bg)] text-muted-foreground',
        dotClassName: 'bg-muted-foreground',
      };
    case 'interrupted':
      return {
        label: t('pages.task.status.interrupted'),
        className: 'bg-[var(--status-neutral-bg)] text-muted-foreground',
        dotClassName: 'bg-muted-foreground',
      };
    default:
      return {
        label: status,
        className: 'bg-[var(--status-neutral-bg)] text-muted-foreground',
        dotClassName: 'bg-muted-foreground',
      };
  }
}

export function getTaskTypeMeta(taskType: string, t: Translate) {
  const normalized = taskType.toLowerCase();

  if (normalized === 'audio_transcription' || normalized.includes('whisper') || normalized.includes('transcription')) {
    return {
      label: t('pages.task.taskType.transcription'),
      icon: Mic,
      iconBg: 'bg-[var(--status-info-bg)]',
      iconColor: 'text-primary',
    };
  }

  if (normalized === 'image_generation' || normalized.includes('diffusion') || normalized.includes('image')) {
    return {
      label: t('pages.task.taskType.imageGeneration'),
      icon: Image,
      iconBg: 'bg-[var(--surface-soft)]',
      iconColor: 'text-[color:var(--accent-foreground)]',
    };
  }

  if (normalized === 'video_generation') {
    return {
      label: t('pages.task.taskType.videoGeneration'),
      icon: Film,
      iconBg: 'bg-[var(--surface-soft)]',
      iconColor: 'text-[color:var(--brand-teal)]',
    };
  }

  if (normalized.includes('download')) {
    return {
      label: t('pages.task.taskType.modelDownload'),
      icon: Download,
      iconBg: 'bg-[var(--surface-soft)]',
      iconColor: 'text-muted-foreground',
    };
  }

  return {
    label: taskType
      .replaceAll('.', ' ')
      .replaceAll('_', ' ')
      .replace(/\b\w/g, (character) => character.toUpperCase()),
    icon: ListChecks,
    iconBg: 'bg-[var(--surface-soft)]',
    iconColor: 'text-muted-foreground',
  };
}

export function isMediaTaskType(taskType: string) {
  return ['image_generation', 'video_generation', 'audio_transcription'].includes(taskType);
}

export function getTaskDeepLink(taskType: string, taskId: string) {
  switch (taskType) {
    case 'image_generation':
      return `/image?task=${taskId}`;
    case 'video_generation':
      return `/video?task=${taskId}`;
    case 'audio_transcription':
      return `/audio?task=${taskId}`;
    default:
      return null;
  }
}
