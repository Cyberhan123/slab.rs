import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const apiClientMock = vi.hoisted(() => ({
  DELETE: vi.fn<() => Promise<unknown>>(),
  GET: vi.fn<() => Promise<unknown>>(),
  PUT: vi.fn<() => Promise<unknown>>(),
}));

const toastMock = vi.hoisted(() => ({
  error: vi.fn<(message: string, options?: unknown) => void>(),
}));

vi.mock('@slab/api', () => ({
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : String(error),
  getLocalizedErrorMessage: (error: unknown) => error instanceof Error ? error.message : String(error),
  apiClient: apiClientMock,
}));
vi.mock('sonner', () => ({
  toast: toastMock,
}));

import { createUiStateStorage, useUiStatePersistenceStatus } from '../ui-state-storage';

describe('createUiStateStorage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUiStatePersistenceStatus.getState().clearFailure();
    apiClientMock.DELETE.mockResolvedValue({ response: { ok: true, status: 204 } });
    apiClientMock.GET.mockResolvedValue({ data: { entries: [] }, response: { status: 200 } });
    apiClientMock.PUT.mockResolvedValue({ response: { ok: true, status: 204 } });
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('loads persisted string values and returns null for missing state', async () => {
    const storage = createUiStateStorage({ namespace: 'test' });

    apiClientMock.GET.mockResolvedValueOnce({
      data: { entries: [{ key: 'test:workspace', value: 'stored' }] },
      response: { status: 200 },
    });
    await expect(storage.getItem('workspace')).resolves.toBe('stored');
    expect(apiClientMock.GET).toHaveBeenCalledWith('/v1/ui-state', {
      params: { query: { keys: 'test:workspace' } },
    });

    apiClientMock.GET.mockResolvedValueOnce({ data: { entries: [] }, response: { status: 200 } });
    await expect(storage.getItem('workspace')).resolves.toBeNull();
    expect(toastMock.error).not.toHaveBeenCalled();
  });

  it('coalesces concurrent reads into a single batched request', async () => {
    const storage = createUiStateStorage({ namespace: 'test' });

    apiClientMock.GET.mockResolvedValueOnce({
      data: {
        entries: [
          { key: 'test:workspace', value: 'w' },
          { key: 'test:audio', value: 'a' },
        ],
      },
      response: { status: 200 },
    });

    const [workspace, audio, missing] = await Promise.all([
      storage.getItem('workspace'),
      storage.getItem('audio'),
      storage.getItem('missing'),
    ]);

    expect(workspace).toBe('w');
    expect(audio).toBe('a');
    expect(missing).toBeNull();
    expect(apiClientMock.GET).toHaveBeenCalledTimes(1);
    expect(apiClientMock.GET).toHaveBeenCalledWith('/v1/ui-state', {
      params: { query: { keys: 'test:workspace,test:audio,test:missing' } },
    });
  });

  it('coalesces pending writes and persists only the latest value', async () => {
    vi.useFakeTimers();
    const storage = createUiStateStorage({ namespace: 'test', writeDelayMs: 50 });

    const firstWrite = storage.setItem('workspace', 'first');
    const secondWrite = storage.setItem('workspace', 'second');

    expect(apiClientMock.PUT).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(50);
    await Promise.all([firstWrite, secondWrite]);

    expect(apiClientMock.PUT).toHaveBeenCalledTimes(1);
    expect(apiClientMock.PUT).toHaveBeenCalledWith('/v1/ui-state/{key}', {
      params: {
        path: {
          key: 'test:workspace',
        },
      },
      body: {
        value: 'second',
      },
    });
  });

  it('removeItem cancels pending writes before deleting state', async () => {
    vi.useFakeTimers();
    const storage = createUiStateStorage({ namespace: 'test', writeDelayMs: 50 });

    const pendingWrite = storage.setItem('workspace', 'draft');
    await storage.removeItem('workspace');
    await pendingWrite;
    await vi.advanceTimersByTimeAsync(50);

    expect(apiClientMock.PUT).not.toHaveBeenCalled();
    expect(apiClientMock.DELETE).toHaveBeenCalledWith('/v1/ui-state/{key}', {
      params: {
        path: {
          key: 'test:workspace',
        },
      },
    });
  });

  it('records load failures while treating missing keys as empty state', async () => {
    const storage = createUiStateStorage({ namespace: 'test' });

    apiClientMock.GET.mockResolvedValueOnce({
      data: null,
      response: { ok: false, status: 500, statusText: 'Server Error' },
    });
    await expect(storage.getItem('workspace')).resolves.toBeNull();

    expect(toastMock.error).toHaveBeenCalledWith('Unable to load UI preferences', {
      description: '500 Server Error',
      id: 'ui-state:load:failed',
    });
    expect(useUiStatePersistenceStatus.getState().lastFailure).toMatchObject({
      key: 'test:workspace',
      message: '500 Server Error',
      operation: 'load',
    });

    toastMock.error.mockClear();
    apiClientMock.GET.mockResolvedValueOnce({ data: { entries: [] }, response: { status: 200 } });
    await expect(storage.getItem('workspace')).resolves.toBeNull();
    expect(toastMock.error).not.toHaveBeenCalled();
  });

  it('swallows persistence failures and resolves storage operations', async () => {
    vi.useFakeTimers();
    const storage = createUiStateStorage({ namespace: 'test', writeDelayMs: 50 });

    apiClientMock.PUT.mockRejectedValueOnce(new Error('write failed'));
    const write = storage.setItem('workspace', 'draft');
    await vi.advanceTimersByTimeAsync(50);
    await expect(write).resolves.toBeUndefined();

    apiClientMock.DELETE.mockRejectedValueOnce(new Error('delete failed'));
    await expect(storage.removeItem('workspace')).resolves.toBeUndefined();
    expect(console.warn).toHaveBeenCalled();
    expect(toastMock.error).toHaveBeenCalledWith('Unable to save UI preferences', {
      description: 'write failed',
      id: 'ui-state:save:failed',
    });
    expect(toastMock.error).toHaveBeenCalledWith('Unable to remove UI preferences', {
      description: 'delete failed',
      id: 'ui-state:remove:failed',
    });
    expect(useUiStatePersistenceStatus.getState().lastFailure).toMatchObject({
      key: 'test:workspace',
      message: 'delete failed',
      operation: 'remove',
    });
  });
});
