import type { StateStorage } from 'zustand/middleware';
import { create } from 'zustand';
import { toast } from 'sonner';

import i18n from '@slab/i18n';
import { apiClient, getLocalizedErrorMessage } from '@slab/api';

type PendingWrite = {
  timer: ReturnType<typeof setTimeout>;
  value: string;
  resolve: Array<() => void>;
};

type UiStateResponse = {
  ok?: boolean;
  status: number;
  statusText?: string;
};

type UiStatePersistenceOperation = 'load' | 'remove' | 'save';

export type UiStatePersistenceFailure = {
  key: string;
  message: string;
  operation: UiStatePersistenceOperation;
  occurredAt: number;
};

type UiStatePersistenceStatus = {
  failureCount: number;
  lastFailure: UiStatePersistenceFailure | null;
  clearFailure: () => void;
  recordFailure: (failure: UiStatePersistenceFailure) => void;
};

export const useUiStatePersistenceStatus = create<UiStatePersistenceStatus>((set) => ({
  failureCount: 0,
  lastFailure: null,
  clearFailure: () => set({ failureCount: 0, lastFailure: null }),
  recordFailure: (failure) =>
    set((current) => ({
      failureCount: current.failureCount + 1,
      lastFailure: failure,
    })),
}));

function toUiStateKey(name: string, namespace: string) {
  return `${namespace}:${name}`;
}

function responseOk(response: UiStateResponse) {
  return typeof response.ok === 'boolean' ? response.ok : response.status < 400;
}

function toHttpError(response: UiStateResponse) {
  const statusText = response.statusText ? ` ${response.statusText}` : '';
  return new Error(`${response.status}${statusText}`);
}

function operationToastTitle(operation: UiStatePersistenceOperation) {
  switch (operation) {
    case 'load':
      return i18n.t('pages.settings.persistence.loadFailed');
    case 'remove':
      return i18n.t('pages.settings.persistence.removeFailed');
    case 'save':
      return i18n.t('pages.settings.persistence.saveFailed');
  }
}

function recordPersistenceFailure(
  operation: UiStatePersistenceOperation,
  key: string,
  error: unknown,
) {
  const message = getLocalizedErrorMessage(error, i18n.t.bind(i18n));
  useUiStatePersistenceStatus.getState().recordFailure({
    key,
    message,
    operation,
    occurredAt: Date.now(),
  });
  toast.error(operationToastTitle(operation), {
    description: message,
    id: `ui-state:${operation}:failed`,
  });
}

function clearPersistenceFailure() {
  useUiStatePersistenceStatus.getState().clearFailure();
}

export function createUiStateStorage(options?: { namespace?: string; writeDelayMs?: number }): StateStorage {
  const namespace = options?.namespace?.trim() || 'zustand';
  const writeDelayMs = options?.writeDelayMs ?? 250;
  const pendingWrites = new Map<string, PendingWrite>();

  const flushWrite = async (key: string) => {
    const pending = pendingWrites.get(key);
    if (!pending) {
      return;
    }

    pendingWrites.delete(key);
    try {
      const result = await apiClient.PUT('/v1/ui-state/{key}', {
        params: {
          path: { key },
        },
        body: { value: pending.value },
      });
      if (!result) {
        return;
      }

      const { response } = result;
      if (!responseOk(response)) {
        throw toHttpError(response);
      }
      clearPersistenceFailure();
    } catch (error) {
      console.warn(`Failed to persist UI state '${key}'.`, error);
      recordPersistenceFailure('save', key, error);
    } finally {
      pending.resolve.forEach((resolve) => resolve());
    }
  };

  const scheduleWrite = (key: string, value: string) =>
    new Promise<void>((resolve) => {
      const pending = pendingWrites.get(key);

      if (pending) {
        clearTimeout(pending.timer);
        pending.value = value;
        pending.resolve.push(resolve);
        pending.timer = setTimeout(() => {
          void flushWrite(key);
        }, writeDelayMs);
        return;
      }

      pendingWrites.set(key, {
        value,
        resolve: [resolve],
        timer: setTimeout(() => {
          void flushWrite(key);
        }, writeDelayMs),
      });
    });

  return {
    getItem: async (name) => {
      const key = toUiStateKey(name, namespace);

      try {
        const result = await apiClient.GET('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });
        if (!result) {
          return null;
        }

        const { data, response } = result;

        if (response.status === 404) {
          return null;
        }

        if (!responseOk(response)) {
          throw toHttpError(response);
        }

        clearPersistenceFailure();
        return typeof data?.value === 'string' ? data.value : null;
      } catch (error) {
        console.warn(`Failed to load UI state '${key}'.`, error);
        recordPersistenceFailure('load', key, error);
        return null;
      }
    },
    setItem: async (name, value) => {
      const key = toUiStateKey(name, namespace);
      await scheduleWrite(key, value);
    },
    removeItem: async (name) => {
      const key = toUiStateKey(name, namespace);
      const pending = pendingWrites.get(key);

      if (pending) {
        clearTimeout(pending.timer);
        pending.resolve.forEach((resolve) => resolve());
        pendingWrites.delete(key);
      }

      try {
        const result = await apiClient.DELETE('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });
        if (!result) {
          return;
        }

        const { response } = result;
        if (!responseOk(response)) {
          throw toHttpError(response);
        }
        clearPersistenceFailure();
      } catch (error) {
        console.warn(`Failed to remove UI state '${key}'.`, error);
        recordPersistenceFailure('remove', key, error);
      }
    },
  };
}
