import type { StateStorage } from 'zustand/middleware';
import { create } from 'zustand';
import { toast } from 'sonner';

import i18n from '@slab/i18n';
import {
  clearUiStateFailureRecord,
  createUiStateStorage as createCoreUiStateStorage,
  getUiStatePersistenceStatus,
  subscribeUiStatePersistence,
  type UiStatePersistenceFailure,
} from '@slab/core/ui-state/server-storage';

export type {
  UiStatePersistenceFailure,
} from '@slab/core/ui-state/server-storage';

type UiStatePersistenceStatus = {
  failureCount: number;
  lastFailure: UiStatePersistenceFailure | null;
  clearFailure: () => void;
  recordFailure: (failure: UiStatePersistenceFailure) => void;
};

function operationToastTitle(operation: UiStatePersistenceFailure['operation']) {
  switch (operation) {
    case 'load':
      return i18n.t('pages.settings.persistence.loadFailed');
    case 'remove':
      return i18n.t('pages.settings.persistence.removeFailed');
    case 'save':
      return i18n.t('pages.settings.persistence.saveFailed');
  }
}

export const useUiStatePersistenceStatus = create<UiStatePersistenceStatus>((set) => ({
  failureCount: getUiStatePersistenceStatus().failureCount,
  lastFailure: null,
  clearFailure: () => {
    clearUiStateFailureRecord();
    set({ failureCount: 0, lastFailure: null });
  },
  recordFailure: (failure) =>
    set((current) => ({
      failureCount: current.failureCount + 1,
      lastFailure: failure,
    })),
}));

/**
 * Module-level side effect: mirror the core persistence observable into the
 * zustand store and surface failures as localized toasts (deduped per
 * operation via a fixed toast id). Runs once on first import.
 */
subscribeUiStatePersistence(() => {
  const { lastFailure } = getUiStatePersistenceStatus();
  if (!lastFailure) return;
  useUiStatePersistenceStatus.getState().recordFailure(lastFailure);
  toast.error(operationToastTitle(lastFailure.operation), {
    description: lastFailure.message,
    id: `ui-state:${lastFailure.operation}:failed`,
  });
});

export function createUiStateStorage(options?: {
  namespace?: string;
  writeDelayMs?: number;
}): StateStorage {
  return createCoreUiStateStorage(options);
}
