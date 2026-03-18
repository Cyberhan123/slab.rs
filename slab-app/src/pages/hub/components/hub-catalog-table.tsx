import { Trash2 } from 'lucide-react';

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
  return (
    <div className="rounded-2xl border border-border/70">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="min-w-[220px]">Model</TableHead>
            <TableHead className="min-w-[280px]">Repository</TableHead>
            <TableHead className="min-w-[260px]">Runtime</TableHead>
            <TableHead className="w-[96px] text-right">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {models.length === 0 ? (
            <TableRow>
              <TableCell colSpan={4} className="py-12 text-center text-muted-foreground">
                No rows on this page.
              </TableCell>
            </TableRow>
          ) : (
            models.map((model) => (
              <TableRow key={model.id}>
                <TableCell className="align-top whitespace-normal">
                  <div className="space-y-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="font-medium">{model.display_name}</p>
                      {model.is_vad_model ? <Badge variant="outline">VAD</Badge> : null}
                    </div>
                    <p className="font-mono text-xs text-muted-foreground" title={model.id}>
                      {model.id}
                    </p>
                  </div>
                </TableCell>

                <TableCell className="align-top whitespace-normal">
                  <p className="break-all font-medium">{model.repo_id}</p>
                  <p
                    className="break-all font-mono text-xs text-muted-foreground"
                    title={model.filename}
                  >
                    {model.filename}
                  </p>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {model.backend_ids.map((backendId) => (
                      <Badge key={backendId} variant="secondary">
                        {formatBackend(backendId)}
                      </Badge>
                    ))}
                  </div>
                </TableCell>

                <TableCell className="align-top whitespace-normal">
                  <StatusBadge status={model.status} />
                  {model.pending ? (
                    <p className="mt-2 text-xs text-muted-foreground">
                      Download is running...
                    </p>
                  ) : null}
                  <p
                    className="mt-2 break-all font-mono text-xs text-muted-foreground"
                    title={model.local_path ?? undefined}
                  >
                    {model.local_path ?? 'Not downloaded yet'}
                  </p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    Updated: {formatDateTime(model.updated_at)}
                  </p>
                </TableCell>

                <TableCell className="align-top text-right">
                  <Button
                    variant="ghost"
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
            ))
          )}
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
