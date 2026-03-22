import { Download, Image, ListChecks, Mic } from 'lucide-react';

export function formatDateTime(value: string) {
  return new Date(value).toLocaleString('en-US', {
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

export function getStatusTone(status: string) {
  switch (status) {
    case 'succeeded':
      return {
        label: 'Succeeded',
        className: 'bg-[#d1fae5] text-[#047857]',
        dotClassName: 'bg-[#10b981]',
      };
    case 'running':
      return {
        label: 'Running',
        className: 'bg-[#dbeafe] text-[#1d4ed8]',
        dotClassName: 'bg-[#3b82f6]',
      };
    case 'failed':
      return {
        label: 'Failed',
        className: 'bg-[#fee2e2] text-[#b91c1c]',
        dotClassName: 'bg-[#ef4444]',
      };
    case 'pending':
      return {
        label: 'Queued',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    case 'cancelled':
      return {
        label: 'Cancelled',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    case 'interrupted':
      return {
        label: 'Interrupted',
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
    default:
      return {
        label: status,
        className: 'bg-[#e5e7eb] text-[#4b5563]',
        dotClassName: 'bg-[#6b7280]',
      };
  }
}

export function getTaskTypeMeta(taskType: string) {
  const normalized = taskType.toLowerCase();

  if (normalized.includes('whisper') || normalized.includes('transcription')) {
    return {
      label: 'Transcription',
      icon: Mic,
      iconBg: 'bg-[#d5e3fd]',
      iconColor: 'text-[#446287]',
    };
  }

  if (normalized.includes('diffusion') || normalized.includes('image')) {
    return {
      label: 'Image Generation',
      icon: Image,
      iconBg: 'bg-[#ede9fe]',
      iconColor: 'text-[#6d28d9]',
    };
  }

  if (normalized.includes('download')) {
    return {
      label: 'Model Download',
      icon: Download,
      iconBg: 'bg-[#e0e3e5]',
      iconColor: 'text-[#5b6872]',
    };
  }

  return {
    label: taskType
      .replaceAll('.', ' ')
      .replaceAll('_', ' ')
      .replace(/\b\w/g, (character) => character.toUpperCase()),
    icon: ListChecks,
    iconBg: 'bg-[#e0e3e5]',
    iconColor: 'text-[#5b6872]',
  };
}
