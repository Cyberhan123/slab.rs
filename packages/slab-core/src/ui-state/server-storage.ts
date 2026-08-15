import { apiClient, getErrorMessage } from '@slab/api';

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

export type UiStatePersistenceStatusData = {
  failureCount: number;
  lastFailure: UiStatePersistenceFailure | null;
};

/**
 * Structurally compatible with zustand's `StateStorage` (declared inline so
 * `@slab/core` does not need a zustand dependency).
 */
export interface SlabStateStorage {
  getItem(name: string): Promise<string | null>;
  setItem(name: string, value: string): Promise<void>;
  removeItem(name: string): Promise<void>;
}

// --- Failure registry (observable; presentation lives in @slab/ui) ---------
// The storage layer records persistence failures as data. Shells subscribe
// and surface them (toast + localized title); tests can assert on the status.

let uiStatePersistenceStatus: UiStatePersistenceStatusData = {
  failureCount: 0,
  lastFailure: null,
};

const uiStatePersistenceListeners = new Set<() => void>();

function notifyUiStatePersistenceListeners() {
  for (const listener of uiStatePersistenceListeners) {
    listener();
  }
}

export function getUiStatePersistenceStatus(): UiStatePersistenceStatusData {
  return uiStatePersistenceStatus;
}

/** Subscribe to persistence-status changes. Returns an unsubscribe fn. */
export function subscribeUiStatePersistence(listener: () => void): () => void {
  uiStatePersistenceListeners.add(listener);
  return () => uiStatePersistenceListeners.delete(listener);
}

/** Clear the recorded failure status (e.g. from the settings page). */
export function clearUiStateFailureRecord(): void {
  uiStatePersistenceStatus = { failureCount: 0, lastFailure: null };
  notifyUiStatePersistenceListeners();
}

function recordPersistenceFailure(
  operation: UiStatePersistenceOperation,
  key: string,
  error: unknown,
) {
  uiStatePersistenceStatus = {
    failureCount: uiStatePersistenceStatus.failureCount + 1,
    lastFailure: {
      key,
      message: getErrorMessage(error),
      operation,
      occurredAt: Date.now(),
    },
  };
  notifyUiStatePersistenceListeners();
}

function clearPersistenceFailure() {
  if (uiStatePersistenceStatus.failureCount === 0) return;
  uiStatePersistenceStatus = { failureCount: 0, lastFailure: null };
  notifyUiStatePersistenceListeners();
}

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

// --- Batched reads ---------------------------------------------------------
// zustand's `persist` middleware calls `getItem` once per store during app
// boot. Issuing one network request per store produces a burst of identical
// GETs, so reads are coalesced: every `getItem` in the same macrotask joins a
// single batched GET `/v1/ui-state?keys=...`, and each waiter is resolved with
// its own key's value (or null when the key has no saved state).
type PendingUiStateBatch = {
  keys: Set<string>;
  promise: Promise<Map<string, string | null>>;
  resolve: (results: Map<string, string | null>) => void;
};

let pendingUiStateBatch: PendingUiStateBatch | null = null;
let uiStateBatchFlushScheduled = false;

function ensurePendingUiStateBatch(): PendingUiStateBatch {
  if (pendingUiStateBatch) {
    return pendingUiStateBatch;
  }
  let resolve!: (results: Map<string, string | null>) => void;
  const promise = new Promise<Map<string, string | null>>((res) => {
    resolve = res;
  });
  pendingUiStateBatch = { keys: new Set(), promise, resolve };
  return pendingUiStateBatch;
}

function scheduleUiStateBatchFlush() {
  if (uiStateBatchFlushScheduled) {
    return;
  }
  uiStateBatchFlushScheduled = true;
  // A macrotask (not a microtask) so reads issued across awaited continuations
  // in the same turn still land in one batch.
  setTimeout(flushUiStateBatch, 0);
}

function flushUiStateBatch() {
  const batch = pendingUiStateBatch;
  pendingUiStateBatch = null;
  uiStateBatchFlushScheduled = false;
  if (!batch || batch.keys.size === 0) {
    return;
  }

  const keys = [...batch.keys];
  apiClient
    .GET('/v1/ui-state', { params: { query: { keys: keys.join(',') } } })
    .then((result) => {
      if (!result) {
        batch.resolve(new Map(keys.map((key) => [key, null])));
        return;
      }
      const { data, response } = result;
      if (!responseOk(response)) {
        throw toHttpError(response);
      }

      const found = new Map<string, string | null>();
      for (const entry of data?.entries ?? []) {
        found.set(entry.key, typeof entry.value === 'string' ? entry.value : null);
      }
      const results = new Map<string, string | null>(
        keys.map((key) => [key, found.get(key) ?? null]),
      );
      clearPersistenceFailure();
      batch.resolve(results);
    })
    .catch((error: unknown) => {
      // Batch failed (e.g. server error). Resolve every waiter with null so
      // hydration still completes with defaults. Listeners dedupe the visible
      // "Unable to load UI preferences" notification across keys.
      for (const key of keys) {
        recordPersistenceFailure('load', key, error);
      }
      batch.resolve(new Map(keys.map((key) => [key, null])));
    });
}

function readUiStateBatched(key: string): Promise<string | null> {
  const batch = ensurePendingUiStateBatch();
  batch.keys.add(key);
  scheduleUiStateBatchFlush();
  return batch.promise.then((results) => results.get(key) ?? null);
}

export function createUiStateStorage(options?: {
  namespace?: string;
  writeDelayMs?: number;
}): SlabStateStorage {
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
        return await readUiStateBatched(key);
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
