/**
 * Shared application configuration derived from environment variables
 * where available, with sensible defaults for local development.
 */

export const DEFAULT_API_BASE_URL = 'http://127.0.0.1:3000';

export function normalizeApiBaseUrl(value?: string | null): string {
  const trimmed = value?.trim();
  const candidate = trimmed && trimmed.length > 0 ? trimmed : DEFAULT_API_BASE_URL;
  return candidate.replace(/\/+$/, '');
}

type ImportMetaWithEnv = ImportMeta & {
  env?: {
    VITE_API_BASE_URL?: string;
  };
};

type GlobalWithApiBaseUrl = typeof globalThis & {
  __SLAB_API_BASE_URL__?: string;
};

function resolveViteApiBaseUrl(): string | undefined {
  return (import.meta as ImportMetaWithEnv).env?.VITE_API_BASE_URL;
}

function resolveRuntimeApiBaseUrl(): string | undefined {
  if (typeof globalThis === 'undefined') {
    return undefined;
  }

  // eslint-disable-next-line no-underscore-dangle
  return (globalThis as GlobalWithApiBaseUrl).__SLAB_API_BASE_URL__;
}

/** Base URL of the slab-server HTTP API. */
export const SERVER_BASE_URL = normalizeApiBaseUrl(
  resolveRuntimeApiBaseUrl() ?? resolveViteApiBaseUrl(),
);
