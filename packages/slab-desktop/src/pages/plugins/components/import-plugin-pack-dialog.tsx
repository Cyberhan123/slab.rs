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

import type { PluginManifestPreview } from '../lib/plugin-manifest-preview';
import { PermissionReviewList } from './permission-review-list';

type ImportPluginPackDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedFileName: string | null;
  setImportFile: (file: File | null) => void;
  canImport: boolean;
  importPending: boolean;
  onImport: () => void;
  importPreview: PluginManifestPreview | null;
  importPreviewFailed: boolean;
  hasReviewedPermissions: boolean;
  onReviewedPermissionsChange: (reviewed: boolean) => void;
};

export function ImportPluginPackDialog({
  open,
  onOpenChange,
  selectedFileName,
  setImportFile,
  canImport,
  importPending,
  onImport,
  importPreview,
  importPreviewFailed,
  hasReviewedPermissions,
  onReviewedPermissionsChange,
}: ImportPluginPackDialogProps) {
  const { t } = useTranslation();
  const showReview = Boolean(selectedFileName) && (importPreview || importPreviewFailed);

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
                data-testid="plugin-import-file-input"
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

          {showReview ? (
            <div className="space-y-3 rounded-2xl border border-border/70 bg-muted/10 p-4">
              {importPreviewFailed ? (
                <p className="rounded-xl border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-700 dark:text-amber-300">
                  {t('pages.plugins.permissions.parseFailed')}
                </p>
              ) : null}
              {importPreview ? <PermissionReviewList preview={importPreview} /> : null}

              <label className="flex items-start gap-2 text-sm" data-testid="plugin-permissions-review">
                <input
                  type="checkbox"
                  className="mt-0.5 size-4 rounded border-border"
                  checked={hasReviewedPermissions}
                  onChange={(event) => onReviewedPermissionsChange(event.target.checked)}
                  disabled={importPending}
                  aria-label={t('pages.plugins.permissions.reviewedCheckbox')}
                  data-testid="plugin-permissions-reviewed-checkbox"
                />
                <span className="text-muted-foreground">
                  {t('pages.plugins.permissions.reviewedCheckbox')}
                </span>
              </label>
            </div>
          ) : null}
        </div>

        <DialogFooter showCloseButton className="border-t border-border/60 px-5 py-4">
          <Button onClick={onImport} disabled={!canImport} data-testid="plugin-import-submit-button">
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
