import type { StateStorage } from 'zustand/middleware';

import { tauriAwareFetch } from '@/lib/api/tauri-transport';
import { SERVER_BASE_URL } from '@/lib/config';

type UiStateValueResponse = {
  key: string;
  value: string;
  updated_at: string;
};

type ErrorPayload = {
  message?: unknown;
};

function buildUiStateUrl(key: string) {
  return new URL(`/v1/ui-state/${encodeURIComponent(key)}`, `${SERVER_BASE_URL}/`);
}

function toUiStateKey(name: string, namespace: string) {
  return `${namespace}:${name}`;
}

async function readErrorMessage(response: Response) {
  try {
    const payload = (await response.json()) as ErrorPayload;
    if (typeof payload.message === 'string' && payload.message.trim()) {
      return payload.message;
    }
  } catch {
    // Ignore invalid error payloads and fall back to the HTTP status.
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
        const response = await tauriAwareFetch(buildUiStateUrl(key), {
          method: 'GET',
        });

        if (response.status === 404) {
          return null;
        }

        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }

        const payload = (await response.json()) as UiStateValueResponse;
        return typeof payload.value === 'string' ? payload.value : null;
      } catch (error) {
        console.warn(`Failed to load UI state '${key}'.`, error);
        return null;
      }
    },
    setItem: async (name, value) => {
      const key = toUiStateKey(name, namespace);

      try {
        const response = await tauriAwareFetch(buildUiStateUrl(key), {
          method: 'PUT',
          headers: {
            'content-type': 'application/json',
          },
          body: JSON.stringify({ value }),
        });

        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
      } catch (error) {
        console.warn(`Failed to persist UI state '${key}'.`, error);
      }
    },
    removeItem: async (name) => {
      const key = toUiStateKey(name, namespace);

      try {
        const response = await tauriAwareFetch(buildUiStateUrl(key), {
          method: 'DELETE',
        });

        if (response.status === 404) {
          return;
        }

        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
      } catch (error) {
        console.warn(`Failed to remove UI state '${key}'.`, error);
      }
    },
  };
}
