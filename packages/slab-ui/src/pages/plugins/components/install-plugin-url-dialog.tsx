import { Loader2, PackagePlus } from 'lucide-react';
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

type InstallPluginUrlDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  pluginId: string;
  packageUrl: string;
  packageSha256: string;
  version: string;
  pending: boolean;
  canInstall: boolean;
  onPluginIdChange: (value: string) => void;
  onPackageUrlChange: (value: string) => void;
  onPackageSha256Change: (value: string) => void;
  onVersionChange: (value: string) => void;
  onInstall: () => void;
};

export function InstallPluginUrlDialog({
  open,
  onOpenChange,
  pluginId,
  packageUrl,
  packageSha256,
  version,
  pending,
  canInstall,
  onPluginIdChange,
  onPackageUrlChange,
  onPackageSha256Change,
  onVersionChange,
  onInstall,
}: InstallPluginUrlDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{t('pages.plugins.dialogs.urlInstall.title')}</DialogTitle>
          <DialogDescription>{t('pages.plugins.dialogs.urlInstall.description')}</DialogDescription>
        </DialogHeader>

        <div className="grid gap-4 py-2">
          <label className="grid gap-2">
            <Label htmlFor="plugin-url-install-id">
              {t('pages.plugins.dialogs.urlInstall.pluginId')}
            </Label>
            <Input
              id="plugin-url-install-id"
              value={pluginId}
              onChange={(event) => onPluginIdChange(event.currentTarget.value)}
              disabled={pending}
              data-testid="plugin-url-install-id"
            />
          </label>

          <label className="grid gap-2">
            <Label htmlFor="plugin-url-install-package-url">
              {t('pages.plugins.dialogs.urlInstall.packageUrl')}
            </Label>
            <Input
              id="plugin-url-install-package-url"
              value={packageUrl}
              onChange={(event) => onPackageUrlChange(event.currentTarget.value)}
              disabled={pending}
              data-testid="plugin-url-install-package-url"
            />
          </label>

          <label className="grid gap-2">
            <Label htmlFor="plugin-url-install-sha">
              {t('pages.plugins.dialogs.urlInstall.packageSha256')}
            </Label>
            <Input
              id="plugin-url-install-sha"
              value={packageSha256}
              onChange={(event) => onPackageSha256Change(event.currentTarget.value)}
              disabled={pending}
              data-testid="plugin-url-install-sha"
            />
          </label>

          <label className="grid gap-2">
            <Label htmlFor="plugin-url-install-version">
              {t('pages.plugins.dialogs.urlInstall.version')}
            </Label>
            <Input
              id="plugin-url-install-version"
              value={version}
              onChange={(event) => onVersionChange(event.currentTarget.value)}
              disabled={pending}
              data-testid="plugin-url-install-version"
            />
          </label>
        </div>

        <DialogFooter showCloseButton>
          <Button onClick={onInstall} disabled={!canInstall} data-testid="plugin-url-install-submit">
            {pending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <PackagePlus className="mr-2 h-4 w-4" />
            )}
            {t('pages.plugins.dialogs.urlInstall.submit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
