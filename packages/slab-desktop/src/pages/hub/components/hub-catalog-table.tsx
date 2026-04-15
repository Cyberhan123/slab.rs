import { Bot, Boxes, Code2, HardDriveDownload, ImageIcon, Loader2, Mic, Settings2, Trash2 } from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { StageEmptyState } from '@slab/components/workspace';

import { canDownloadModel, type ModelItem } from '../hooks/use-hub-model-catalog';
import { StatusBadge } from './status-badge';

type HubCatalogTableProps = {
  models: ModelItem[];
  deletePending: boolean;
  onDownloadClick: (model: ModelItem) => void;
  onEnhanceClick: (model: ModelItem) => void;
  onDeleteClick: (model: ModelItem) => void;
};

export function HubCatalogTable({
  models,
  deletePending,
  onDownloadClick,
  onEnhanceClick,
  onDeleteClick,
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
          onDownloadClick={onDownloadClick}
          onEnhanceClick={onEnhanceClick}
          onDeleteClick={onDeleteClick}
        />
      ))}
    </div>
  );
}

function HubModelCard({
  model,
  deletePending,
  onDownloadClick,
  onEnhanceClick,
  onDeleteClick,
}: {
  model: ModelItem;
  deletePending: boolean;
  onDownloadClick: (model: ModelItem) => void;
  onEnhanceClick: (model: ModelItem) => void;
  onDeleteClick: (model: ModelItem) => void;
}) {
  const { t, i18n } = useTranslation();
  const Icon = getModelIcon(model);
  const backendLabel = model.backend_ids[0]
    ? formatBackend(model.backend_ids[0], t)
    : t('pages.hub.catalog.runtime');
  const showDownloadAction = model.pending || canDownloadModel(model);
  const sourceLabel = model.local_path ?? model.repo_id ?? model.id;

  return (
    <article
      className="group relative overflow-hidden rounded-[30px] border border-border/40 bg-[color:color-mix(in_oklab,var(--surface-1)_92%,var(--background))] p-6 shadow-[0_24px_56px_-40px_color-mix(in_oklab,var(--foreground)_40%,transparent)]"
    >
      <div className="absolute inset-0 opacity-70 [background:radial-gradient(circle_at_top_right,color-mix(in_oklab,var(--brand-teal)_9%,transparent),transparent_34%),radial-gradient(circle_at_bottom_left,color-mix(in_oklab,var(--brand-gold)_12%,transparent),transparent_30%)]" />

      <div className="relative flex h-full flex-col gap-6">
        <div className="flex items-start justify-between gap-4">
          <div className="flex size-14 items-center justify-center rounded-[18px] bg-[var(--surface-soft)] text-[var(--brand-teal)]">
            <Icon className="size-6" />
          </div>

          <StatusBadge status={model.status} />
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
                className="size-10 rounded-full border border-border/70 bg-[var(--shell-card)]/80"
                onClick={() => onEnhanceClick(model)}
                disabled={model.pending}
                aria-label={t('pages.hub.catalog.actions.enhanceAria', { model: model.display_name })}
              >
                <Settings2 className="size-4" />
              </Button>
              <Button
                variant="quiet"
                size="icon-sm"
                className="size-10 rounded-full border border-border/70 bg-[var(--shell-card)]/80 text-destructive hover:bg-[var(--shell-card)] hover:text-destructive"
                onClick={() => onDeleteClick(model)}
                disabled={deletePending || model.pending}
                aria-label={t('pages.hub.catalog.actions.deleteAria', { model: model.display_name })}
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
          </div>

          <div className="mt-auto flex flex-col gap-2 pt-1">
            {showDownloadAction ? (
              <div className="flex flex-wrap items-center gap-3 rounded-2xl border border-border/60 bg-[var(--shell-card)]/65 px-3 py-3">
                <Button
                  variant={model.pending ? 'pill' : 'cta'}
                  size="sm"
                  onClick={() => onDownloadClick(model)}
                  disabled={model.pending}
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
              </div>
            ) : null}

            <div className="space-y-1">
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
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
                <span className="font-semibold text-[var(--brand-teal)]">
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
