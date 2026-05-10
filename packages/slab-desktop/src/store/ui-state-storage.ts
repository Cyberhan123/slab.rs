import type { StateStorage } from 'zustand/middleware';

import { apiClient } from '@slab/api';

type PendingWrite = {
  timer: ReturnType<typeof setTimeout>;
  value: string;
  resolve: Array<() => void>;
};

function toUiStateKey(name: string, namespace: string) {
  return `${namespace}:${name}`;
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
      await apiClient.PUT('/v1/ui-state/{key}', {
        params: {
          path: { key },
        },
        body: { value: pending.value },
      });
    } catch (error) {
      console.warn(`Failed to persist UI state '${key}'.`, error);
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
        const { data, response } = await apiClient.GET('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });

        if (response.status === 404) {
          return null;
        }

        return typeof data?.value === 'string' ? data.value : null;
      } catch (error) {
        console.warn(`Failed to load UI state '${key}'.`, error);
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
        await apiClient.DELETE('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });
      } catch (error) {
        console.warn(`Failed to remove UI state '${key}'.`, error);
      }
    },
  };
}
