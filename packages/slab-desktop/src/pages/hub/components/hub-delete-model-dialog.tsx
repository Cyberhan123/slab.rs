import { Loader2, Trash2 } from 'lucide-react';
import { useTranslation, Trans } from '@slab/i18n';

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@slab/components/alert-dialog';

import type { ModelItem } from '../hooks/use-hub-model-catalog';

type HubDeleteModelDialogProps = {
  model: ModelItem | null;
  open: boolean;
  pending: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
};

export function HubDeleteModelDialog({
  model,
  open,
  pending,
  onOpenChange,
  onConfirm,
}: HubDeleteModelDialogProps) {
  const { t } = useTranslation();
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{t('pages.hub.dialogs.delete.title')}</AlertDialogTitle>
          <AlertDialogDescription>
            {model ? (
              <Trans
                i18nKey="pages.hub.dialogs.delete.descriptionWithModel"
                values={{ model: model.display_name }}
                components={{ strong: <strong /> }}
              />
            ) : (
              t('pages.hub.dialogs.delete.descriptionFallback')
            )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>
            {t('pages.hub.dialogs.delete.cancel')}
          </AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={pending}
            onClick={(event) => {
              event.preventDefault();
              onConfirm();
            }}
          >
            {pending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Trash2 className="mr-2 h-4 w-4" />
            )}
            {t('pages.hub.dialogs.delete.confirm')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
