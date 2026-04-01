import { Loader2, Trash2 } from 'lucide-react';

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
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Delete model entry?</AlertDialogTitle>
          <AlertDialogDescription>
            {model ? (
              <>
                Remove <strong>{model.display_name}</strong> from the model catalog and delete its
                stored JSON config. This does not delete any downloaded model file on disk.
              </>
            ) : (
              'Remove this model entry from the catalog.'
            )}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={pending}>Cancel</AlertDialogCancel>
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
            Delete entry
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
