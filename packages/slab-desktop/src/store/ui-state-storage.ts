import type { StateStorage } from 'zustand/middleware';

import { apiClient } from '@/lib/api';

function toUiStateKey(name: string, namespace: string) {
  return `${namespace}:${name}`;
}

export function createUiStateStorage(options?: { namespace?: string }): StateStorage {
  const namespace = options?.namespace?.trim() || 'zustand';

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

      try {
        await apiClient.PUT('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
          body: { value },
        });
      } catch (error) {
        console.warn(`Failed to persist UI state '${key}'.`, error);
      }
    },
    removeItem: async (name) => {
      const key = toUiStateKey(name, namespace);

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
