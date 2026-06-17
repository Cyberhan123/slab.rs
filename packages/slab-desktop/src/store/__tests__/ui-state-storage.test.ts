import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const apiClientMock = vi.hoisted(() => ({
  DELETE: vi.fn<() => Promise<unknown>>(),
  GET: vi.fn<() => Promise<unknown>>(),
  PUT: vi.fn<() => Promise<unknown>>(),
}));

vi.mock('@slab/api', () => ({
  apiClient: apiClientMock,
}));

import { createUiStateStorage } from '../ui-state-storage';

describe('createUiStateStorage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    apiClientMock.DELETE.mockResolvedValue({});
    apiClientMock.GET.mockResolvedValue({ data: null, response: { status: 404 } });
    apiClientMock.PUT.mockResolvedValue({});
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('loads persisted string values and returns null for missing state', async () => {
    const storage = createUiStateStorage({ namespace: 'test' });

    apiClientMock.GET.mockResolvedValueOnce({ data: { value: 'stored' }, response: { status: 200 } });
    await expect(storage.getItem('workspace')).resolves.toBe('stored');
    expect(apiClientMock.GET).toHaveBeenCalledWith('/v1/ui-state/{key}', {
      params: {
        path: {
          key: 'test:workspace',
        },
      },
    });

    apiClientMock.GET.mockResolvedValueOnce({ data: null, response: { status: 404 } });
    await expect(storage.getItem('workspace')).resolves.toBeNull();
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
  });
});
