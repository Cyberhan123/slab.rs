/**
 * Shared application configuration derived from environment variables
 * where available, with sensible defaults for local development.
 */

/** Base URL of the slab-server HTTP API. */
export const SERVER_BASE_URL: string =
  import.meta.env.VITE_API_BASE_URL ?? 'http://localhost:3000';
