import { Loader2 } from 'lucide-react';

import { Progress } from '@slab/components/progress';
import { cn } from '@/lib/utils';
import type { GenerationProgress } from '@/lib/media-task-api';

type GenerationProgressProps = {
  progress: GenerationProgress | null;
  labels: {
    eta: string;
    finalizing: string;
    queued: string;
    running: string;
    step: string;
    title: string;
  };
  className?: string;
  testId?: string;
};

export function GenerationProgressView({
  progress,
  labels,
  className,
  testId,
}: GenerationProgressProps) {
  if (!progress) {
    return null;
  }

  const percent = progress.percent ?? 0;
  const stageLabel =
    progress.stage === 'queued'
      ? labels.queued
      : progress.stage === 'finalizing'
        ? labels.finalizing
        : labels.running;

  return (
    <div
      className={cn(
        'space-y-3 rounded-[18px] border border-border/60 bg-glass-bg-strong px-4 py-4',
        className,
      )}
      data-testid={testId}
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-full bg-[color:color-mix(in_oklab,var(--brand-teal)_12%,transparent)] text-[color:var(--brand-teal)]">
          <Loader2 className="size-4 animate-spin" />
        </div>
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <p className="text-sm font-semibold text-foreground">{labels.title}</p>
            <span className="text-xs font-medium text-muted-foreground">
              {Math.round(percent)}%
            </span>
          </div>
          <p className="text-xs leading-5 text-muted-foreground">
            {stageLabel}
            {progress.stepLabel ? ` · ${progress.stepLabel}` : ''}
          </p>
        </div>
      </div>

      <Progress value={percent} className="h-2 bg-[var(--surface-soft)]" />

      <div className="flex flex-wrap items-center justify-between gap-2 text-caption font-medium text-muted-foreground">
        <span>
          {labels.step}: {progress.stepLabel ?? '—'}
        </span>
        <span>
          {labels.eta}: {formatDuration(progress.etaMs)}
        </span>
      </div>
    </div>
  );
}

function formatDuration(value: number | null) {
  if (value === null || !Number.isFinite(value) || value <= 0) {
    return '—';
  }

  const seconds = Math.max(Math.round(value / 1000), 1);
  if (seconds < 60) {
    return `${seconds}s`;
  }

  const minutes = Math.floor(seconds / 60);
  const remaining = seconds % 60;
  if (minutes < 60) {
    return remaining > 0 ? `${minutes}m ${remaining}s` : `${minutes}m`;
  }

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}
