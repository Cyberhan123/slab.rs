import {
  Bot,
  Boxes,
  Code2,
  HardDriveDownload,
  ImageIcon,
  Loader2,
  Mic,
  Play,
  Power,
  PowerOff,
  Repeat2,
  Settings2,
  Trash2,
} from 'lucide-react';
import { clamp } from 'lodash-es';
import { useTranslation } from '@slab/i18n';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { Progress } from '@slab/components/progress';
import { StageEmptyState } from '@slab/components/workspace';
import { cn } from '@/lib/utils';

import {
  canDownloadModel,
  canRunModelLifecycleAction,
  getModelUseRoute,
  type ModelItem,
} from '../hooks/use-hub-model-catalog';
import { StatusBadge } from './status-badge';

type HubCatalogTableProps = {
  models: ModelItem[];
  deletePending: boolean;
  modelActionPending: boolean;
  modelActionPendingId: string | null;
  modelActionErrors: Record<string, string>;
  onDownloadClick: (model: ModelItem) => void;
  onEnhanceClick: (model: ModelItem) => void;
  onDeleteClick: (model: ModelItem) => void;
  onLoadClick: (model: ModelItem) => void;
  onSwitchClick: (model: ModelItem) => void;
  onUnloadClick: (model: ModelItem) => void;
  onUseClick: (model: ModelItem, route: string) => void;
};

export function HubCatalogTable({
  models,
  deletePending,
  modelActionPending,
  modelActionPendingId,
  modelActionErrors,
  onDownloadClick,
  onEnhanceClick,
  onDeleteClick,
  onLoadClick,
  onSwitchClick,
  onUnloadClick,
  onUseClick,
}: HubCatalogTableProps) {
  const { t } = useTranslation();
  if (models.length === 0) {
    return (
      <StageEmptyState
        icon={Boxes}
        title={t('pages.hub.catalog.emptyPageTitle')}
        description={t('pages.hub.catalog.emptyPageDescription')}
        className="min-h-[280px]"
      />
    );
  }

  return (
    <div className="grid gap-5 md:grid-cols-2 2xl:grid-cols-3">
      {models.map((model) => (
        <HubModelCard
          key={model.id}
          model={model}
          deletePending={deletePending}
          modelActionPending={modelActionPending}
          modelActionPendingId={modelActionPendingId}
          modelActionError={modelActionErrors[model.id] ?? null}
          onDownloadClick={onDownloadClick}
          onEnhanceClick={onEnhanceClick}
          onDeleteClick={onDeleteClick}
          onLoadClick={onLoadClick}
          onSwitchClick={onSwitchClick}
          onUnloadClick={onUnloadClick}
          onUseClick={onUseClick}
        />
      ))}
    </div>
  );
}

function HubModelCard({
  model,
  deletePending,
  modelActionPending,
  modelActionPendingId,
  modelActionError,
  onDownloadClick,
  onEnhanceClick,
  onDeleteClick,
  onLoadClick,
  onSwitchClick,
  onUnloadClick,
  onUseClick,
}: {
  model: ModelItem;
  deletePending: boolean;
  modelActionPending: boolean;
  modelActionPendingId: string | null;
  modelActionError: string | null;
  onDownloadClick: (model: ModelItem) => void;
  onEnhanceClick: (model: ModelItem) => void;
  onDeleteClick: (model: ModelItem) => void;
  onLoadClick: (model: ModelItem) => void;
  onSwitchClick: (model: ModelItem) => void;
  onUnloadClick: (model: ModelItem) => void;
  onUseClick: (model: ModelItem, route: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const Icon = getModelIcon(model);
  const backendLabel = model.backend_ids[0]
    ? formatBackend(model.backend_ids[0], t)
    : t('pages.hub.catalog.runtime');
  const showDownloadAction = model.pending || canDownloadModel(model);
  const sourceLabel = model.local_path ?? model.repo_id ?? model.id;
  const downloadProgress = model.download_progress;
  const downloadProgressValue = getDownloadProgressValue(downloadProgress);
  const downloadProgressLabel =
    getDownloadProgressLabel(downloadProgress) || t('pages.hub.catalog.downloadRunning');
  const downloadProgressSummary =
    getDownloadProgressSummary(downloadProgress) || t('pages.hub.catalog.downloading');
  const runtimeStateLabel = getRuntimeStateLabel(model, t);
  const useRoute = getModelUseRoute(model);
  const lifecycleAvailable = canRunModelLifecycleAction(model);
  const lifecycleBusy = modelActionPendingId === model.id;
  const actionDisabled = modelActionPending || model.pending;
  const isLoaded = Boolean(model.runtime_state?.loaded);

  return (
    <article
      data-testid={`hub-model-card-${model.id}`}
      className="group relative overflow-hidden rounded-3xl border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-1)_92%,var(--background))] p-6"
    >
      <div className="absolute inset-0 opacity-70 [background:radial-gradient(circle_at_top_right,color-mix(in_oklab,var(--brand-teal)_9%,transparent),transparent_34%),radial-gradient(circle_at_bottom_left,color-mix(in_oklab,var(--brand-gold)_12%,transparent),transparent_30%)]" />

      <div className="relative flex h-full flex-col gap-6">
        <div className="flex items-start justify-between gap-4">
          <div className="flex size-14 items-center justify-center rounded-[18px] bg-[var(--surface-soft)] text-[color:var(--brand-teal)]">
            <Icon className="size-6" />
          </div>

          <div className="flex flex-wrap items-center justify-end gap-2">
            <StatusBadge status={model.status} />
            {runtimeStateLabel ? (
              <Badge
                variant="chip"
                className="bg-[var(--surface-1)] px-3 py-1 text-micro font-bold uppercase tracking-eyebrow text-muted-foreground"
              >
                {runtimeStateLabel}
              </Badge>
            ) : null}
          </div>
        </div>

        <div className="flex min-w-0 flex-1 flex-col gap-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0 space-y-2">
              <h3 className="text-[1.9rem] font-semibold tracking-tight text-foreground">
                {model.display_name}
              </h3>
              <p className="max-w-2xl text-sm leading-6 text-muted-foreground">
                {describeModel(model, t)}
              </p>
            </div>

            <div className="flex items-center gap-2">
              <Button
                variant="quiet"
                size="icon-sm"
                className="size-10 rounded-full border border-border/70 bg-glass-bg-strong"
                onClick={() => onEnhanceClick(model)}
                disabled={model.pending}
                aria-label={t('pages.hub.catalog.actions.enhanceAria', { model: model.display_name })}
                data-testid={`hub-model-enhance-${model.id}`}
              >
                <Settings2 className="size-4" />
              </Button>
              <Button
                variant="quiet"
                size="icon-sm"
                className="size-10 rounded-full border border-border/70 bg-glass-bg-strong text-destructive hover:bg-[var(--shell-card)] hover:text-destructive"
                onClick={() => onDeleteClick(model)}
                disabled={deletePending || model.pending}
                aria-label={t('pages.hub.catalog.actions.deleteAria', { model: model.display_name })}
                data-testid={`hub-model-delete-${model.id}`}
              >
                <Trash2 className="size-4" />
              </Button>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
              {backendLabel}
            </Badge>
            <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
              {formatKind(model.kind, t)}
            </Badge>
            {model.is_vad_model ? (
              <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
                {t('pages.hub.catalog.vad')}
              </Badge>
            ) : null}
            {model.filename ? (
              <Badge
                variant="chip"
                className="bg-[var(--surface-1)] px-3 py-1 font-mono text-muted-foreground"
              >
                {shortFileName(model.filename)}
              </Badge>
            ) : null}
            <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
              {t('pages.hub.catalog.size', {
                value: formatBytesOrUnknown(model.size_bytes, t('pages.hub.catalog.unknownSize')),
              })}
            </Badge>
            <Badge
              variant="chip"
              className={cn(
                'bg-[var(--surface-1)] px-3 py-1 text-muted-foreground',
                model.vram_risk === 'high' && 'text-destructive',
              )}
            >
              {t(`pages.hub.catalog.vramRisk.${model.vram_risk}`)}
            </Badge>
          </div>

          <div className="mt-auto flex flex-col gap-2 pt-1">
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant={useRoute ? 'cta' : 'pill'}
                size="sm"
                disabled={!useRoute || model.pending}
                onClick={() => {
                  if (useRoute) {
                    onUseClick(model, useRoute);
                  }
                }}
                data-testid={`hub-model-use-${model.id}`}
              >
                <Play className="size-4" />
                {useRoute ? t('pages.hub.catalog.actions.use') : t('pages.hub.catalog.actions.useDisabled')}
              </Button>

              {lifecycleAvailable ? (
                <>
                  {isLoaded ? (
                    <Button
                      variant="pill"
                      size="sm"
                      disabled={actionDisabled}
                      onClick={() => onUnloadClick(model)}
                      data-testid={`hub-model-unload-${model.id}`}
                    >
                      {lifecycleBusy ? (
                        <Loader2 className="size-4 animate-spin" />
                      ) : (
                        <PowerOff className="size-4" />
                      )}
                      {t('pages.hub.catalog.actions.unload')}
                    </Button>
                  ) : (
                    <Button
                      variant="pill"
                      size="sm"
                      disabled={actionDisabled}
                      onClick={() => onLoadClick(model)}
                      data-testid={`hub-model-load-${model.id}`}
                    >
                      {lifecycleBusy ? (
                        <Loader2 className="size-4 animate-spin" />
                      ) : (
                        <Power className="size-4" />
                      )}
                      {t('pages.hub.catalog.actions.load')}
                    </Button>
                  )}

                  <Button
                    variant="pill"
                    size="sm"
                    disabled={actionDisabled || Boolean(model.runtime_state?.active)}
                    onClick={() => onSwitchClick(model)}
                    data-testid={`hub-model-switch-${model.id}`}
                  >
                    {lifecycleBusy ? (
                      <Loader2 className="size-4 animate-spin" />
                    ) : (
                      <Repeat2 className="size-4" />
                    )}
                    {t('pages.hub.catalog.actions.switch')}
                  </Button>
                </>
              ) : null}
            </div>

            {modelActionError ? (
              <p
                className="rounded-2xl border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs leading-5 text-destructive"
                data-testid={`hub-model-action-error-${model.id}`}
              >
                {modelActionError}
              </p>
            ) : null}

            {showDownloadAction ? (
              <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-border/60 bg-glass-bg px-3 py-3">
                <Button
                  variant={model.pending ? 'pill' : 'cta'}
                  size="sm"
                  onClick={() => onDownloadClick(model)}
                  disabled={model.pending}
                  data-testid={`hub-model-download-${model.id}`}
                >
                  {model.pending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <HardDriveDownload className="size-4" />
                  )}
                  {model.pending
                    ? t('pages.hub.catalog.downloading')
                    : t('pages.hub.catalog.download')}
                </Button>
                <p className="flex-1 text-xs leading-5 text-muted-foreground">
                  {model.pending
                    ? t('pages.hub.catalog.downloadPendingDescription')
                    : t('pages.hub.catalog.downloadIdleDescription')}
                </p>
                {model.pending ? (
                  <div className="w-full space-y-2">
                    <div className="flex flex-wrap items-center justify-between gap-2 text-caption font-medium text-muted-foreground">
                      <span className="truncate" title={downloadProgressLabel}>
                        {downloadProgressLabel}
                      </span>
                      <span>{downloadProgressSummary}</span>
                    </div>
                    <Progress
                      value={downloadProgressValue ?? 0}
                      className={cn(
                        'h-2 bg-[var(--surface-soft)]',
                        downloadProgressValue === null && 'animate-pulse'
                      )}
                    />
                  </div>
                ) : null}
              </div>
            ) : null}

            <div className="space-y-1">
              <p className="text-caption font-semibold uppercase tracking-eyebrow text-muted-foreground">
                {t('pages.hub.catalog.source')}
              </p>
              <p className="truncate font-mono text-xs text-muted-foreground" title={sourceLabel}>
                {sourceLabel}
              </p>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
              <span>
                {t('pages.hub.catalog.updatedAt', {
                  value: formatDateTime(
                    model.updated_at,
                    i18n.resolvedLanguage ?? i18n.language,
                    t('pages.hub.catalog.unknownTime'),
                  ),
                })}
              </span>
              {model.pending ? (
                <span className="font-semibold text-[color:var(--brand-teal)]">
                  {t('pages.hub.catalog.downloadRunning')}
                </span>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </article>
  );
}

function describeModel(
  model: ModelItem,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  const backendLabel = model.backend_ids[0]
    ? formatBackend(model.backend_ids[0], t).toLowerCase()
    : t('pages.hub.catalog.runtime').toLowerCase();

  if (model.pending) {
    return t('pages.hub.catalog.descriptions.pending', { backend: backendLabel });
  }

  if (model.local_path) {
    return t('pages.hub.catalog.descriptions.local', { backend: backendLabel });
  }

  return t('pages.hub.catalog.descriptions.imported', {
    backend: backendLabel,
    repo: model.repo_id || t('pages.hub.catalog.configuredRepository'),
  });
}

function getRuntimeStateLabel(
  model: ModelItem,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  const runtimeState = model.runtime_state;
  if (!runtimeState || (!model.local_path && !runtimeState.loaded && !runtimeState.active)) {
    return null;
  }

  if (runtimeState.active) {
    return t('pages.hub.catalog.runtimeState.active');
  }

  if (runtimeState.loaded) {
    return t('pages.hub.catalog.runtimeState.loaded');
  }

  return t('pages.hub.catalog.runtimeState.unloaded');
}

function formatBackend(
  id: string,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  switch (id) {
    case 'ggml.llama':
      return t('pages.hub.catalog.backend.llama');
    case 'ggml.whisper':
      return t('pages.hub.catalog.backend.whisper');
    case 'ggml.diffusion':
      return t('pages.hub.catalog.backend.diffusion');
    default:
      return id;
  }
}

function formatKind(
  kind: 'local' | 'cloud',
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  return kind === 'cloud'
    ? t('pages.hub.catalog.kind.cloud')
    : t('pages.hub.catalog.kind.local');
}

function shortFileName(filename: string) {
  return filename.split('/').at(-1) ?? filename;
}

function getModelIcon(model: ModelItem) {
  const haystack = `${model.display_name} ${model.repo_id} ${model.filename}`.toLowerCase();

  if (
    model.capabilities.includes('video_generation') ||
    model.capabilities.includes('image_generation') ||
    haystack.includes('image') ||
    haystack.includes('diffusion')
  ) {
    return ImageIcon;
  }

  if (
    model.capabilities.includes('audio_transcription') ||
    model.capabilities.includes('audio_vad') ||
    haystack.includes('audio') ||
    haystack.includes('whisper')
  ) {
    return Mic;
  }

  if (haystack.includes('coder') || haystack.includes('code')) {
    return Code2;
  }

  return Bot;
}

function formatDateTime(
  value: string | null | undefined,
  locale: string,
  fallback: string,
) {
  if (!value) {
    return fallback;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString(locale, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function getDownloadProgressValue(progress: ModelItem['download_progress']) {
  if (!progress?.total || progress.total <= 0) {
    return null;
  }

  return clamp((progress.current / progress.total) * 100, 0, 100);
}

function getDownloadProgressLabel(progress: ModelItem['download_progress']) {
  if (!progress) {
    return '';
  }

  const label = progress.label?.trim() || 'download';
  if (progress.step && progress.stepCount) {
    return `${label} (${progress.step}/${progress.stepCount})`;
  }

  return label;
}

function getDownloadProgressSummary(progress: ModelItem['download_progress']) {
  if (!progress) {
    return '';
  }

  const current = formatBytes(progress.current);
  if (!progress.total || progress.total <= 0) {
    return current;
  }

  const percentage = Math.round((progress.current / progress.total) * 100);
  return `${percentage}% · ${current} / ${formatBytes(progress.total)}`;
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return '0 B';
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const exponent = clamp(Math.floor(Math.log(value) / Math.log(1024)), 0, units.length - 1);
  const size = value / 1024 ** exponent;
  let fractionDigits = 2;
  if (size >= 100 || exponent === 0) {
    fractionDigits = 0;
  } else if (size >= 10) {
    fractionDigits = 1;
  }
  return `${size.toFixed(fractionDigits)} ${units[exponent]}`;
}

function formatBytesOrUnknown(value: number | null, fallback: string) {
  return value && value > 0 ? formatBytes(value) : fallback;
}
