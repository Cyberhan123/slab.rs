/**
 * Boot-time readiness gate for the slab-server HTTP API.
 *
 * The desktop shell spawns slab-server as a sidecar and opens the webview
 * immediately; the HTTP listener only binds several seconds later, so the
 * first request burst (zustand ui-state hydration, react-query prefetches)
 * races the listener and fails with `ERR_CONNECTION_REFUSED`.
 * `waitForApiServer` polls the `/health` endpoint until the server answers,
 * letting callers defer app boot until requests will actually succeed.
 */

import { SERVER_BASE_URL, normalizeApiBaseUrl } from "./config";

export const DEFAULT_HEALTH_PATH = "/health";

export type WaitForApiServerOptions = {
  /** API origin to probe. Defaults to the shared `SERVER_BASE_URL`. */
  baseUrl?: string | null;
  /** Health endpoint path (default `/health`). */
  path?: string;
  /** Delay between probes (default 250ms). */
  intervalMs?: number;
  /** Give up after this long and resolve `false` (default 30s). */
  timeoutMs?: number;
  /** Fetch implementation (default `fetch`); injectable for tests. */
  fetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  /** Aborts waiting early (resolves `false`). */
  signal?: AbortSignal;
};

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal?.aborted) {
      resolve();
      return;
    }
    const onAbort = () => {
      clearTimeout(timer);
      resolve();
    };
    const timer = setTimeout(() => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}

/**
 * Poll the health endpoint until the server is reachable and healthy.
 *
 * Resolves `true` on the first 2xx response and `false` on timeout or abort —
 * never throws, so callers can fall back to booting regardless (the pre-gate
 * behavior) instead of leaving a dead window.
 */
export async function waitForApiServer(options: WaitForApiServerOptions = {}): Promise<boolean> {
  const baseUrl = normalizeApiBaseUrl(options.baseUrl ?? SERVER_BASE_URL);
  const rawPath = options.path ?? DEFAULT_HEALTH_PATH;
  const endpoint = `${baseUrl}${rawPath.startsWith("/") ? rawPath : `/${rawPath}`}`;
  const intervalMs = Math.max(0, options.intervalMs ?? 250);
  const timeoutMs = Math.max(0, options.timeoutMs ?? 30_000);
  const fetchImpl = options.fetch ?? fetch;
  const deadline = Date.now() + timeoutMs;

  // Sequential by design: this is a readiness poll, not a parallel workload.
  for (;;) {
    if (options.signal?.aborted) {
      return false;
    }
    try {
      // eslint-disable-next-line no-await-in-loop
      const response = await fetchImpl(endpoint, { method: "GET" });
      if (response.ok) {
        return true;
      }
    } catch {
      // Not listening yet (connection refused) — retry after the interval.
    }
    const remaining = deadline - Date.now();
    if (remaining <= 0) {
      return false;
    }
    // eslint-disable-next-line no-await-in-loop
    await sleep(Math.min(intervalMs, remaining), options.signal);
  }
}
