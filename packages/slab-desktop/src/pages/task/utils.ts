import { Download, Image, ListChecks, Mic } from 'lucide-react';

type Translate = (key: string, options?: Record<string, unknown>) => string;

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

export function isSettledStatus(status: string) {
  return ['succeeded', 'failed', 'cancelled', 'interrupted'].includes(status);
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
        className: 'bg-[var(--status-success-bg)] text-[var(--success)]',
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

  if (normalized.includes('whisper') || normalized.includes('transcription')) {
    return {
      label: t('pages.task.taskType.transcription'),
      icon: Mic,
      iconBg: 'bg-[var(--status-info-bg)]',
      iconColor: 'text-primary',
    };
  }

  if (normalized.includes('diffusion') || normalized.includes('image')) {
    return {
      label: t('pages.task.taskType.imageGeneration'),
      icon: Image,
      iconBg: 'bg-[var(--surface-soft)]',
      iconColor: 'text-[var(--accent-foreground)]',
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
