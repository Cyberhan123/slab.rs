import { Bot, Boxes, Code2, ImageIcon, Mic, Trash2 } from 'lucide-react';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { StageEmptyState } from '@slab/components/workspace';

import type { ModelItem } from '../hooks/use-hub-model-catalog';
import { StatusBadge } from './status-badge';

type HubCatalogTableProps = {
  models: ModelItem[];
  deletePending: boolean;
  onDeleteClick: (model: ModelItem) => void;
};

export function HubCatalogTable({ models, deletePending, onDeleteClick }: HubCatalogTableProps) {
  if (models.length === 0) {
    return (
      <StageEmptyState
        icon={Boxes}
        title="No cards on this page"
        description="Try another page or relax the active filters."
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
          onDeleteClick={onDeleteClick}
        />
      ))}
    </div>
  );
}

function HubModelCard({
  model,
  deletePending,
  onDeleteClick,
}: {
  model: ModelItem;
  deletePending: boolean;
  onDeleteClick: (model: ModelItem) => void;
}) {
  const Icon = getModelIcon(model);
  const backendLabel = model.backend_ids[0] ? formatBackend(model.backend_ids[0]) : 'Runtime';
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
              <p className="max-w-2xl text-sm leading-6 text-muted-foreground">{describeModel(model)}</p>
            </div>

            <Button
              variant="quiet"
              size="icon-sm"
              className="size-10 rounded-full border border-border/70 bg-[var(--shell-card)]/80 text-destructive hover:bg-[var(--shell-card)] hover:text-destructive"
              onClick={() => onDeleteClick(model)}
              disabled={deletePending}
              aria-label={`Delete ${model.display_name}`}
            >
              <Trash2 className="size-4" />
            </Button>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
              {backendLabel}
            </Badge>
            <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
              {formatProvider(model.provider)}
            </Badge>
            {model.is_vad_model ? (
              <Badge variant="chip" className="bg-[var(--surface-1)] px-3 py-1 text-muted-foreground">
                VAD
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
            <div className="space-y-1">
              <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                Source
              </p>
              <p className="truncate font-mono text-xs text-muted-foreground" title={sourceLabel}>
                {sourceLabel}
              </p>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-muted-foreground">
              <span>Updated {formatDateTime(model.updated_at)}</span>
              {model.pending ? (
                <span className="font-semibold text-[var(--brand-teal)]">
                  Download task is running
                </span>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </article>
  );
}

function describeModel(model: ModelItem) {
  const backendLabel = model.backend_ids[0] ? formatBackend(model.backend_ids[0]).toLowerCase() : 'runtime';

  if (model.pending) {
    return `This ${backendLabel} entry is syncing into the local runtime catalog. Once the download finishes, the runtime path and readiness state will update automatically.`;
  }

  if (model.local_path) {
    return `Local ${backendLabel} model ready for inference. The manifest is already connected to a runtime path and can be used without leaving this workspace.`;
  }

  return `Imported ${backendLabel} manifest from ${model.repo_id || 'the configured repository'}. Review the catalog entry, backend mapping, and file before pulling it into local storage.`;
}

function formatBackend(id: string) {
  switch (id) {
    case 'ggml.llama':
      return 'Llama';
    case 'ggml.whisper':
      return 'Whisper';
    case 'ggml.diffusion':
      return 'Diffusion';
    default:
      return id;
  }
}

function formatProvider(provider: string) {
  return provider
    .replace(/^local\./, '')
    .replace(/[._-]+/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

function shortFileName(filename: string) {
  return filename.split('/').at(-1) ?? filename;
}

function getModelIcon(model: ModelItem) {
  const haystack = `${model.display_name} ${model.repo_id} ${model.filename}`.toLowerCase();

  if (
    model.backend_ids.includes('ggml.diffusion') ||
    haystack.includes('image') ||
    haystack.includes('diffusion')
  ) {
    return ImageIcon;
  }

  if (
    model.backend_ids.includes('ggml.whisper') ||
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

function formatDateTime(value?: string | null) {
  if (!value) {
    return 'Unknown';
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}
