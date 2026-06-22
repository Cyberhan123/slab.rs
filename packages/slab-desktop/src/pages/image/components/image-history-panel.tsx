import { Download, ImageIcon, RotateCcw } from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@slab/components/dialog';
import { resolveMediaUrl, type ImageGenerationTask } from '@/lib/media-task-api';

type ImageHistoryPanelProps = {
  handleDownload: (src: string, index: number) => void;
  history: ImageGenerationTask[];
  historyDialogOpen: boolean;
  historyError: string | null;
  historyLoading: boolean;
  openHistoryDetail: (taskId: string) => void | Promise<void>;
  refillFromHistory: (task: ImageGenerationTask) => void;
  selectedHistoryTask: ImageGenerationTask | null;
  setHistoryDialogOpen: (open: boolean) => void;
  setSelectedHistoryTask: (task: ImageGenerationTask | null) => void;
};

function formatHistoryTime(value: string) {
  return new Date(value).toLocaleString(undefined, {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function ImageHistoryPanel({
  handleDownload,
  history,
  historyDialogOpen,
  historyError,
  historyLoading,
  openHistoryDetail,
  refillFromHistory,
  selectedHistoryTask,
  setHistoryDialogOpen,
  setSelectedHistoryTask,
}: ImageHistoryPanelProps) {
  const { t } = useTranslation();
  const selectedHistoryImages =
    selectedHistoryTask?.image_urls
      .map((url) => resolveMediaUrl(url))
      .filter((url): url is string => typeof url === 'string' && url.length > 0) ?? [];

  return (
    <>
      <div className="border-t border-border/60 bg-[var(--surface-soft)] px-5 py-4 xl:px-8">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <p className="text-caption font-bold uppercase tracking-eyebrow text-muted-foreground">
              {t('pages.image.history.title')}
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              {historyLoading
                ? t('pages.image.history.loading')
                : historyError
                  ? t('pages.image.history.error', { message: historyError })
                  : t('pages.image.history.description')}
            </p>
          </div>
          <Badge variant="chip">{history.length}</Badge>
        </div>

        <div className="mt-3 grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {history.slice(0, 3).map((task) => {
            const previewUrl = resolveMediaUrl(task.primary_image_url ?? task.image_urls[0]);
            return (
              <button
                key={task.task_id}
                type="button"
                data-testid={`image-history-item-${task.task_id}`}
                className="group flex gap-3 rounded-[18px] border border-border/50 bg-[var(--shell-card)] p-3 text-left transition hover:border-[var(--brand-teal)]/50 hover:shadow-elevation-2"
                onClick={() => void openHistoryDetail(task.task_id)}
              >
                <div className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-[14px] bg-[var(--surface-soft)] text-muted-foreground">
                  {previewUrl ? (
                    <img src={previewUrl} alt={task.prompt} className="h-full w-full object-cover" />
                  ) : (
                    <ImageIcon className="size-5" />
                  )}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="line-clamp-2 text-sm font-semibold leading-5 text-foreground">
                    {task.prompt}
                  </p>
                  <div className="mt-2 flex flex-wrap items-center gap-2 text-caption text-muted-foreground">
                    <span className="rounded-full bg-[var(--surface-soft)] px-2 py-0.5">
                      {task.status}
                    </span>
                    <span>{formatHistoryTime(task.created_at)}</span>
                  </div>
                </div>
              </button>
            );
          })}
          {!historyLoading && history.length === 0 ? (
            <div className="rounded-[18px] border border-dashed border-border/60 bg-[var(--shell-card)] px-4 py-5 text-sm text-muted-foreground md:col-span-2 xl:col-span-3">
              {t('pages.image.history.empty')}
            </div>
          ) : null}
        </div>
      </div>

      <Dialog
        open={historyDialogOpen}
        onOpenChange={(open) => {
          setHistoryDialogOpen(open);
          if (!open) {
            setSelectedHistoryTask(null);
          }
        }}
      >
        <DialogContent className="max-w-5xl">
          {selectedHistoryTask ? (
            <>
              <DialogHeader>
                <DialogTitle>{t('pages.image.history.detailTitle')}</DialogTitle>
                <DialogDescription>
                  {selectedHistoryTask.status} | {formatHistoryTime(selectedHistoryTask.created_at)}
                </DialogDescription>
              </DialogHeader>
              <div className="flex flex-wrap justify-end gap-2">
                <Button
                  type="button"
                  variant="pill"
                  size="sm"
                  data-testid="image-history-refill"
                  onClick={() => refillFromHistory(selectedHistoryTask)}
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  {t('pages.image.history.actions.refill')}
                </Button>
              </div>
              <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
                <div className="grid max-h-[70vh] gap-3 overflow-y-auto sm:grid-cols-2">
                  {selectedHistoryImages.map((src, index) => (
                    <figure
                      key={src}
                      className="overflow-hidden rounded-[22px] border border-border/60 bg-[var(--surface-soft)]"
                    >
                      <img src={src} alt={selectedHistoryTask.prompt} className="w-full object-cover" />
                      <figcaption className="flex justify-end border-t border-border/50 bg-[var(--shell-card)] px-3 py-2">
                        <Button variant="pill" size="sm" onClick={() => handleDownload(src, index)}>
                          <Download className="h-3.5 w-3.5" />
                          {t('pages.image.workbench.gallery.downloadAria')}
                        </Button>
                      </figcaption>
                    </figure>
                  ))}
                </div>
                <div className="space-y-4 rounded-[22px] bg-[var(--surface-soft)] p-4">
                  <div>
                    <p className="text-caption font-bold uppercase tracking-eyebrow text-muted-foreground">
                      {t('pages.image.workbench.prompt.label')}
                    </p>
                    <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-foreground">
                      {selectedHistoryTask.prompt}
                    </p>
                  </div>
                  <div className="grid grid-cols-2 gap-3 text-sm">
                    <div>
                      <p className="text-xs text-muted-foreground">
                        {t('pages.image.history.fields.mode')}
                      </p>
                      <p className="font-semibold">{selectedHistoryTask.mode}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">
                        {t('pages.image.history.fields.size')}
                      </p>
                      <p className="font-semibold">
                        {selectedHistoryTask.width} x {selectedHistoryTask.height}
                      </p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">
                        {t('pages.image.history.fields.backend')}
                      </p>
                      <p className="truncate font-semibold">{selectedHistoryTask.backend_id}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">
                        {t('pages.image.history.fields.model')}
                      </p>
                      <p className="truncate font-semibold">
                        {selectedHistoryTask.model_id ?? selectedHistoryTask.model_path}
                      </p>
                    </div>
                  </div>
                  {selectedHistoryTask.error_msg ? (
                    <p className="rounded-xl bg-destructive/10 p-3 text-xs leading-5 text-destructive">
                      {selectedHistoryTask.error_msg}
                    </p>
                  ) : null}
                </div>
              </div>
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </>
  );
}
