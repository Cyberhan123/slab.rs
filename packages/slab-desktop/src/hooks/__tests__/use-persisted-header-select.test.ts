import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../store/ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

import { useHeaderUiStore } from '@/store/useHeaderUiStore';

import { usePersistedHeaderSelect } from '../use-persisted-header-select';

const modelOptions = [
  { id: 'model-a' },
  { id: 'model-b' },
  { id: 'model-c' },
];

describe('usePersistedHeaderSelect', () => {
  beforeEach(() => {
    useHeaderUiStore.setState({
      hasHydrated: false,
      selections: {},
    });
  });

  it('waits for persisted state hydration before selecting a fallback option', async () => {
    renderHook(() =>
      usePersistedHeaderSelect({
        key: 'assistant:model',
        options: modelOptions,
      }),
    );

    expect(useHeaderUiStore.getState().selections).toEqual({});

    act(() => {
      useHeaderUiStore.getState().setHasHydrated(true);
    });

    await waitFor(() => {
      expect(useHeaderUiStore.getState().selections['assistant:model']).toBe('model-a');
    });
  });

  it('uses the preferred default when the persisted value is stale', async () => {
    useHeaderUiStore.setState({
      hasHydrated: true,
      selections: {
        'assistant:model': 'model-disabled',
      },
    });

    renderHook(() =>
      usePersistedHeaderSelect({
        key: 'assistant:model',
        options: [
          { id: 'model-disabled', disabled: true },
          ...modelOptions,
        ],
        getDefaultValue: () => 'model-b',
      }),
    );

    await waitFor(() => {
      expect(useHeaderUiStore.getState().selections['assistant:model']).toBe('model-b');
    });
  });

  it('clears a persisted value when no enabled options are available', async () => {
    useHeaderUiStore.setState({
      hasHydrated: true,
      selections: {
        'assistant:model': 'model-a',
      },
    });

    renderHook(() =>
      usePersistedHeaderSelect({
        key: 'assistant:model',
        options: [{ id: 'model-a', disabled: true }],
      }),
    );

    await waitFor(() => {
      expect(useHeaderUiStore.getState().selections['assistant:model']).toBeUndefined();
    });
  });

  it('exposes a setter backed by the header UI store', () => {
    useHeaderUiStore.setState({
      hasHydrated: true,
      selections: {},
    });

    const { result } = renderHook(() =>
      usePersistedHeaderSelect({
        key: 'assistant:model',
        options: modelOptions,
      }),
    );

    act(() => {
      result.current.setValue(' model-c ');
    });

    expect(useHeaderUiStore.getState().selections['assistant:model']).toBe('model-c');
  });
});
