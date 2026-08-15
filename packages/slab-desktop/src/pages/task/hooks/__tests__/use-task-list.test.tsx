import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

vi.mock('@mantine/hooks', () => ({
  useInterval: vi.fn<() => { start: () => void; stop: () => void }>(() => ({
    start: vi.fn<() => void>(),
    stop: vi.fn<() => void>(),
  })),
}));

const { useQueryMock, useMutationMock } = vi.hoisted(() => ({
  useQueryMock: vi.fn<() => unknown>(),
  useMutationMock: vi.fn<() => unknown>(),
}));

vi.mock('@slab/api', () => ({
  default: { useQuery: useQueryMock, useMutation: useMutationMock },
}));
vi.mock('sonner', () => ({ toast: { error: vi.fn<(message: string) => void>() } }));
vi.mock('@slab/i18n', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
  translateServerField: (_i18n: unknown, _field: string, fallback: string) => fallback,
}));
vi.mock('@slab/core/api/error-description', () => ({ getErrorDescription: () => 'error' }));

import type { Task } from '../../const';
import { useTaskList } from '../use-task-list';

function task(overrides: { status: string } & Record<string, unknown>): Task {
  return {
    id: 'task-1',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:30Z',
    ...overrides,
  } as unknown as Task;
}

function mutationResult() {
  return {
    isPending: false,
    mutateAsync: vi.fn<(payload: unknown) => Promise<unknown>>().mockResolvedValue(undefined),
  };
}

describe('useTaskList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useQueryMock.mockReturnValue({
      data: [],
      error: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<unknown>>().mockResolvedValue(undefined),
    });
    useMutationMock.mockReturnValue(mutationResult());
  });

  it('subscribes to the tasks endpoint and lifecycle mutations with toasts suppressed', async () => {
    await renderHook(() => useTaskList());

    expect(useQueryMock).toHaveBeenCalledWith('get', '/v1/tasks');
    expect(useMutationMock).toHaveBeenCalledWith('get', '/v1/tasks/{id}', {
      meta: { skipGlobalErrorToast: true },
    });
    expect(useMutationMock).toHaveBeenCalledWith('get', '/v1/tasks/{id}/result', {
      meta: { skipGlobalErrorToast: true },
    });
    expect(useMutationMock).toHaveBeenCalledWith('post', '/v1/tasks/{id}/cancel', {
      meta: { skipGlobalErrorToast: true },
    });
    expect(useMutationMock).toHaveBeenCalledWith('post', '/v1/tasks/{id}/restart', {
      meta: { skipGlobalErrorToast: true },
    });
  });

  it('derives metrics, pagination and the current page from loaded tasks', async () => {
    useQueryMock.mockReturnValue({
      data: Array.from({ length: 5 }, (_, index) =>
        task({ id: `t${index}`, status: index < 3 ? 'succeeded' : 'running' }),
      ),
      error: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<unknown>>().mockResolvedValue(undefined),
    });

    const { result } = await renderHook(() => useTaskList());

    await vi.waitFor(() => expect(result.current.metrics.total).toBe(5));
    expect(result.current.metrics).toEqual({
      total: 5,
      running: 2,
      queued: 0,
      failed: 0,
      succeeded: 3,
    });
    expect(result.current.totalPages).toBe(2);
    expect(result.current.currentPage).toBe(1);
    expect(result.current.paginatedTasks).toHaveLength(4);
  });

  it('exposes the lifecycle mutations on its return surface', async () => {
    const { result } = await renderHook(() => useTaskList());

    expect(result.current.cancelTaskMutation).toBeDefined();
    expect(result.current.restartTaskMutation).toBeDefined();
    expect(typeof result.current.fetchTaskDetail).toBe('function');
  });
});
