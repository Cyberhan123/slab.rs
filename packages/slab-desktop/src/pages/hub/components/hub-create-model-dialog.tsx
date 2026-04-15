import { Loader2, Plus, Upload } from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@slab/components/dialog';
import { Button } from '@slab/components/button';
import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';

type HubCreateModelDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedFileName: string | null;
  setCreateFile: (file: File | null) => void;
  canCreate: boolean;
  createPending: boolean;
  onCreate: () => void;
};

export function HubCreateModelDialog({
  open,
  onOpenChange,
  selectedFileName,
  setCreateFile,
  canCreate,
  createPending,
  onCreate,
}: HubCreateModelDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] max-w-3xl gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b border-border/60 px-5 pt-5 pb-4">
          <DialogTitle>{t('pages.hub.dialogs.create.title')}</DialogTitle>
          <DialogDescription>
            {t('pages.hub.dialogs.create.description')}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 overflow-y-auto px-5 py-4">
          <div className="space-y-3 rounded-2xl border border-border/70 bg-muted/10 p-4">
            <div className="grid gap-2">
              <Label htmlFor="hub-model-config-file">
                {t('pages.hub.dialogs.create.modelPackLabel')}
              </Label>
              <Input
                id="hub-model-config-file"
                type="file"
                accept=".slab"
                onChange={(event) => setCreateFile(event.target.files?.[0] ?? null)}
                disabled={createPending}
              />
            </div>

            {selectedFileName ? (
              <div className="rounded-xl border border-border/70 bg-background px-3 py-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Upload className="h-4 w-4 text-primary" />
                  <span className="truncate">{selectedFileName}</span>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {t('pages.hub.dialogs.create.selectedDescription')}
                </p>
              </div>
            ) : (
              <div className="rounded-xl border border-dashed border-border/70 bg-background px-4 py-6 text-center text-sm text-muted-foreground">
                <Upload className="mx-auto mb-3 h-5 w-5" />
                {t('pages.hub.dialogs.create.emptyDescription')}
              </div>
            )}
          </div>
        </div>

        <DialogFooter showCloseButton className="border-t border-border/60 px-5 py-4">
          <Button onClick={onCreate} disabled={!canCreate}>
            {createPending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Plus className="mr-2 h-4 w-4" />
            )}
            {t('pages.hub.dialogs.create.submit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
