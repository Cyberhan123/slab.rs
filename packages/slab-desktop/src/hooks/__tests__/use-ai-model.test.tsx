import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../store/ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

const { mockUseMutation, mockUseQuery } = vi.hoisted(() => ({
  mockUseMutation: vi.fn<(...args: unknown[]) => unknown>(),
  mockUseQuery: vi.fn<(...args: unknown[]) => unknown>(),
}));

vi.mock('@slab/api', () => ({
  default: {
    useMutation: mockUseMutation,
    useQuery: mockUseQuery,
  },
}));

import { useHeaderUiStore } from '@/store/useHeaderUiStore';
import { toAiModelList, useAiModel, type UnifiedModelResponse } from '../use-ai-model';

function model(overrides: Partial<UnifiedModelResponse> = {}): UnifiedModelResponse {
  return {
    backend_id: null,
    capabilities: ['chat_generation'],
    chat_capabilities: null,
    created_at: '2026-01-01T00:00:00Z',
    display_name: 'Model',
    id: 'model-a',
    kind: 'local',
    runtime_presets: null,
    runtime_state: null,
    spec: {
      context_window: null,
      filename: 'model.gguf',
      local_path: null,
      provider_id: null,
      remote_model_id: null,
      repo_id: 'owner/model',
    },
    status: 'not_downloaded',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function installApiMocks(data: UnifiedModelResponse[]) {
  const mutateAsync = vi.fn<() => Promise<unknown>>().mockResolvedValue({});

  mockUseQuery.mockReturnValue({
    data,
    error: null,
    isLoading: false,
    isRefetching: false,
    refetch: vi.fn<() => Promise<{ data: UnifiedModelResponse[] }>>().mockResolvedValue({ data }),
  });
  mockUseMutation.mockReturnValue({
    isPending: false,
    mutateAsync,
  });

  return { mutateAsync };
}

describe('useAiModel', () => {
  beforeEach(() => {
    mockUseMutation.mockReset();
    mockUseQuery.mockReset();
    useHeaderUiStore.setState({
      hasHydrated: true,
      selections: {},
    });
  });

  it('normalizes catalog payloads and filters invalid records', () => {
    const normalized = toAiModelList([
      model({
        backend_id: 'ggml.llama',
        status: 'ready',
        spec: {
          context_window: 4096,
          filename: 'model.gguf',
          local_path: '/models/model.gguf',
          provider_id: null,
          remote_model_id: null,
          repo_id: 'owner/model',
        },
      }),
      { id: 123 },
    ]);

    expect(normalized).toHaveLength(1);
    expect(normalized[0]).toMatchObject({
      backend_ids: ['ggml.llama'],
      filename: 'model.gguf',
      local_path: '/models/model.gguf',
      pending: false,
      repo_id: 'owner/model',
      status: 'ready',
    });
  });

  it('persists a fallback selection after hydration', async () => {
    installApiMocks([model({ id: 'model-a' }), model({ id: 'model-b' })]);

    renderHook(() =>
      useAiModel({
        capability: 'chat_generation',
        storageKey: 'assistant:model',
      }),
    );

    await waitFor(() => {
      expect(useHeaderUiStore.getState().selections['assistant:model']).toBe('model-a');
    });
  });

  it('uses the preferred default when persisted selection is stale', async () => {
    useHeaderUiStore.setState({
      hasHydrated: true,
      selections: {
        'image:model': 'missing-model',
      },
    });
    installApiMocks([
      model({ id: 'model-a' }),
      model({
        id: 'model-b',
        spec: {
          context_window: null,
          filename: 'model-b.gguf',
          local_path: '/models/model-b.gguf',
          provider_id: null,
          remote_model_id: null,
          repo_id: 'owner/model-b',
        },
      }),
    ]);

    renderHook(() =>
      useAiModel({
        capability: 'image_generation',
        storageKey: 'image:model',
        getDefaultModelId: (models) => models.find((item) => Boolean(item.local_path))?.id,
      }),
    );

    await waitFor(() => {
      expect(useHeaderUiStore.getState().selections['image:model']).toBe('model-b');
    });
  });

  it('keeps non-persisted selection in local hook state', () => {
    installApiMocks([model({ id: 'model-a' })]);

    const { result } = renderHook(() => useAiModel());

    act(() => {
      result.current.setSelectedId('model-a');
    });

    expect(result.current.selectedId).toBe('model-a');
    expect(useHeaderUiStore.getState().selections).toEqual({});
  });
});
