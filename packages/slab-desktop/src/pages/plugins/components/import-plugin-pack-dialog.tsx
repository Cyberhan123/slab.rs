import { Loader2, PackagePlus, Upload } from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Button } from '@slab/components/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@slab/components/dialog';
import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';

type ImportPluginPackDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedFileName: string | null;
  setImportFile: (file: File | null) => void;
  canImport: boolean;
  importPending: boolean;
  onImport: () => void;
};

export function ImportPluginPackDialog({
  open,
  onOpenChange,
  selectedFileName,
  setImportFile,
  canImport,
  importPending,
  onImport,
}: ImportPluginPackDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] max-w-3xl gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b border-border/60 px-5 pt-5 pb-4">
          <DialogTitle>{t('pages.plugins.dialogs.import.title')}</DialogTitle>
          <DialogDescription>{t('pages.plugins.dialogs.import.description')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-5 overflow-y-auto px-5 py-4">
          <div className="space-y-3 rounded-2xl border border-border/70 bg-muted/10 p-4">
            <div className="grid gap-2">
              <Label htmlFor="plugin-pack-file">{t('pages.plugins.dialogs.import.packLabel')}</Label>
              <Input
                id="plugin-pack-file"
                type="file"
                accept=".plugin.slab"
                onChange={(event) => setImportFile(event.target.files?.[0] ?? null)}
                disabled={importPending}
              />
            </div>

            {selectedFileName ? (
              <div className="rounded-xl border border-border/70 bg-background px-3 py-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <Upload className="h-4 w-4 text-primary" />
                  <span className="truncate">{selectedFileName}</span>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {t('pages.plugins.dialogs.import.selectedDescription')}
                </p>
              </div>
            ) : (
              <div className="rounded-xl border border-dashed border-border/70 bg-background px-4 py-6 text-center text-sm text-muted-foreground">
                <Upload className="mx-auto mb-3 h-5 w-5" />
                {t('pages.plugins.dialogs.import.emptyDescription')}
              </div>
            )}
          </div>
        </div>

        <DialogFooter showCloseButton className="border-t border-border/60 px-5 py-4">
          <Button onClick={onImport} disabled={!canImport}>
            {importPending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <PackagePlus className="mr-2 h-4 w-4" />
            )}
            {t('pages.plugins.dialogs.import.submit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
