import type { StateStorage } from 'zustand/middleware';

import { apiClient } from '@/lib/api';

type ErrorPayload = {
  error?: unknown;
  message?: unknown;
};

type UiStateApiClient = {
  DELETE: (
    path: '/v1/ui-state/{key}',
    init: {
      params: {
        path: {
          key: string;
        };
      };
    },
  ) => Promise<{
    error?: ErrorPayload;
    response: Response;
  }>;
  GET: (
    path: '/v1/ui-state/{key}',
    init: {
      params: {
        path: {
          key: string;
        };
      };
    },
  ) => Promise<{
    data?: { value?: string };
    error?: ErrorPayload;
    response: Response;
  }>;
  PUT: (
    path: '/v1/ui-state/{key}',
    init: {
      body: {
        value: string;
      };
      params: {
        path: {
          key: string;
        };
      };
    },
  ) => Promise<{
    error?: ErrorPayload;
    response: Response;
  }>;
};

const uiStateApiClient = apiClient as unknown as UiStateApiClient;

function toUiStateKey(name: string, namespace: string) {
  return `${namespace}:${name}`;
}

function readErrorMessage(error: unknown, response: Response) {
  if (typeof error === 'object' && error !== null) {
    const payload = error as ErrorPayload;
    if (typeof payload.message === 'string' && payload.message.trim()) {
      return payload.message;
    }

    if (typeof payload.error === 'string' && payload.error.trim()) {
      return payload.error;
    }
  }

  const fallback = `${response.status} ${response.statusText}`.trim();
  return fallback || 'Request failed';
}

export function createUiStateStorage(options?: { namespace?: string }): StateStorage {
  const namespace = options?.namespace?.trim() || 'zustand';

  return {
    getItem: async (name) => {
      const key = toUiStateKey(name, namespace);

      try {
        const { data, error, response } = await uiStateApiClient.GET('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });

        if (response.status === 404) {
          return null;
        }

        if (!response.ok) {
          throw new Error(readErrorMessage(error, response));
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
        const { error, response } = await uiStateApiClient.PUT('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
          body: { value },
        });

        if (!response.ok) {
          throw new Error(readErrorMessage(error, response));
        }
      } catch (error) {
        console.warn(`Failed to persist UI state '${key}'.`, error);
      }
    },
    removeItem: async (name) => {
      const key = toUiStateKey(name, namespace);

      try {
        const { error, response } = await uiStateApiClient.DELETE('/v1/ui-state/{key}', {
          params: {
            path: { key },
          },
        });

        if (response.status === 404) {
          return;
        }

        if (!response.ok) {
          throw new Error(readErrorMessage(error, response));
        }
      } catch (error) {
        console.warn(`Failed to remove UI state '${key}'.`, error);
      }
    },
  };
}
