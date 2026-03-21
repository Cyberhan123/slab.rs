import { Download, Trash2 } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { StageEmptyState } from '@/components/ui/workspace';

import type { ModelItem } from '../hooks/use-hub-model-catalog';
import { StatusBadge } from './status-badge';

type HubCatalogTableProps = {
  models: ModelItem[];
  deletePending: boolean;
  onDeleteClick: (model: ModelItem) => void;
};

export function HubCatalogTable({
  models,
  deletePending,
  onDeleteClick,
}: HubCatalogTableProps) {
  if (models.length === 0) {
    return (
      <StageEmptyState
        icon={Download}
        title="No rows on this page"
        description="Try a different page or update filters."
        className="min-h-[280px]"
      />
    );
  }

  return (
    <div className="workspace-soft-panel overflow-hidden rounded-[28px] p-2">
      <Table>
        <TableHeader className="[&_tr]:border-b-border/65 bg-[var(--surface-1)]">
          <TableRow className="hover:bg-transparent">
            <TableHead className="h-12 min-w-[240px] px-4 text-xs uppercase tracking-[0.12em] text-muted-foreground">
              Model
            </TableHead>
            <TableHead className="h-12 min-w-[280px] px-4 text-xs uppercase tracking-[0.12em] text-muted-foreground">
              Repository
            </TableHead>
            <TableHead className="h-12 min-w-[280px] px-4 text-xs uppercase tracking-[0.12em] text-muted-foreground">
              Runtime
            </TableHead>
            <TableHead className="h-12 w-[120px] px-4 text-right text-xs uppercase tracking-[0.12em] text-muted-foreground">
              Actions
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {models.map((model) => (
            <TableRow key={model.id} className="border-b-border/50 hover:bg-[var(--surface-soft)]/60">
              <TableCell className="px-4 py-4 align-top whitespace-normal">
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="font-medium tracking-tight">{model.display_name}</p>
                    {model.is_vad_model ? <Badge variant="chip">VAD</Badge> : null}
                  </div>
                  <p className="font-mono text-xs text-muted-foreground/90" title={model.id}>
                    {model.id}
                  </p>
                </div>
              </TableCell>

              <TableCell className="px-4 py-4 align-top whitespace-normal">
                <p className="break-all font-medium">{model.repo_id}</p>
                <p className="mt-1 break-all font-mono text-xs text-muted-foreground" title={model.filename}>
                  {model.filename}
                </p>
                <div className="mt-3 flex flex-wrap gap-2">
                  {model.backend_ids.map((backendId) => (
                    <Badge key={backendId} variant="chip">
                      {formatBackend(backendId)}
                    </Badge>
                  ))}
                </div>
              </TableCell>

              <TableCell className="px-4 py-4 align-top whitespace-normal">
                <StatusBadge status={model.status} />
                {model.pending ? (
                  <p className="mt-2 text-xs text-muted-foreground">Download task is running...</p>
                ) : null}
                <p
                  className="mt-2 break-all font-mono text-xs text-muted-foreground"
                  title={model.local_path ?? undefined}
                >
                  {model.local_path ?? 'Not downloaded yet'}
                </p>
                <p className="mt-2 text-xs text-muted-foreground">
                  Updated {formatDateTime(model.updated_at)}
                </p>
              </TableCell>

              <TableCell className="px-4 py-4 align-top text-right">
                <Button
                  variant="quiet"
                  size="sm"
                  className="text-destructive hover:text-destructive"
                  onClick={() => onDeleteClick(model)}
                  disabled={deletePending}
                >
                  <Trash2 className="mr-2 h-4 w-4" />
                  Delete
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
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
