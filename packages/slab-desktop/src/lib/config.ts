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

/** Base URL of the slab-server HTTP API. */
export const SERVER_BASE_URL = normalizeApiBaseUrl(import.meta.env.VITE_API_BASE_URL);
