import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const apiMock = vi.hoisted(() => ({
  useQuery: vi.fn<(...args: unknown[]) => unknown>(),
}));
const toastMock = vi.hoisted(() => ({
  error: vi.fn<(message: string, options?: unknown) => void>(),
}));

vi.mock('@slab/api', () => ({
  default: apiMock,
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : String(error),
}));
vi.mock('sonner', () => ({
  toast: toastMock,
}));

import { useMediaTaskPolling } from '../use-media-task-polling';

function renderPolling(taskId: string | null = 'task-1') {
  return renderHook(() =>
    useMediaTaskPolling({
      enabled: true,
      intervalMs: 2_000,
      pollingErrorToastId: 'media-poll-error',
      taskId,
      toPollingErrorMessage: (message) => `Polling failed: ${message}`,
    }),
  );
}

function latestQueryOptions() {
  const call = apiMock.useQuery.mock.calls.at(-1);
  expect(call).toBeDefined();
  return call?.[3] as {
    enabled: boolean;
    refetchInterval: number | false;
    retry: boolean;
  };
}

describe('useMediaTaskPolling', () => {
  let queryState: {
    data: unknown;
    dataUpdatedAt: number;
    error: unknown;
    errorUpdatedAt: number;
  };

  beforeEach(() => {
    vi.clearAllMocks();
    queryState = {
      data: undefined,
      dataUpdatedAt: 0,
      error: null,
      errorUpdatedAt: 0,
    };
    apiMock.useQuery.mockImplementation(() => queryState);
  });

  it('disables polling when no task id is available', () => {
    renderPolling(null);

    expect(latestQueryOptions()).toMatchObject({
      enabled: false,
      refetchInterval: false,
      retry: false,
    });
  });

  it('backs off polling failures and dedupes the toast id', async () => {
    const { rerender } = renderPolling();

    expect(latestQueryOptions()).toMatchObject({
      enabled: true,
      refetchInterval: 2_000,
      retry: false,
    });

    queryState = {
      data: undefined,
      dataUpdatedAt: 0,
      error: new Error('temporary outage'),
      errorUpdatedAt: 1,
    };
    rerender();

    await waitFor(() => {
      expect(toastMock.error).toHaveBeenCalledWith('Polling failed: temporary outage', {
        id: 'media-poll-error',
      });
      expect(latestQueryOptions().refetchInterval).toBe(4_000);
    });

    queryState = {
      ...queryState,
      errorUpdatedAt: 2,
    };
    rerender();

    await waitFor(() => expect(latestQueryOptions().refetchInterval).toBe(8_000));
  });
});
