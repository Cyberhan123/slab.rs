import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useHeaderUiStore } from '../useHeaderUiStore';

// Mock the UI state storage
vi.mock('../ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

describe('useHeaderUiStore', () => {
  beforeEach(() => {
    useHeaderUiStore.setState({
      selections: {},
      hasHydrated: false,
    });
  });

  it('should have initial state', () => {
    const state = useHeaderUiStore.getState();
    expect(state.selections).toEqual({});
    expect(state.hasHydrated).toBe(false);
  });

  it('should set selection', () => {
    useHeaderUiStore.getState().setSelection('model-select', 'model-123');
    expect(useHeaderUiStore.getState().selections['model-select']).toBe('model-123');
  });

  it('should trim whitespace from selection key and value', () => {
    useHeaderUiStore.getState().setSelection('  model-select  ', '  model-123  ');
    expect(useHeaderUiStore.getState().selections['model-select']).toBe('model-123');
  });

  it('should not set selection for empty key', () => {
    useHeaderUiStore.getState().setSelection('', 'model-123');
    expect(useHeaderUiStore.getState().selections).toEqual({});
  });

  it('should remove selection when value is empty', () => {
    const state = useHeaderUiStore.getState();
    state.setSelection('model-select', 'model-123');
    state.setSelection('model-select', '');
    expect(useHeaderUiStore.getState().selections['model-select']).toBeUndefined();
  });

  it('should clear selection', () => {
    const state = useHeaderUiStore.getState();
    state.setSelection('model-select', 'model-123');
    state.clearSelection('model-select');
    expect(useHeaderUiStore.getState().selections['model-select']).toBeUndefined();
  });

  it('should handle clearing non-existent selection', () => {
    useHeaderUiStore.getState().clearSelection('non-existent');
    expect(useHeaderUiStore.getState().selections).toEqual({});
  });

  it('should set hasHydrated state', () => {
    useHeaderUiStore.getState().setHasHydrated(true);
    expect(useHeaderUiStore.getState().hasHydrated).toBe(true);
  });

  it('should maintain multiple selections', () => {
    const state = useHeaderUiStore.getState();
    state.setSelection('model-select', 'model-123');
    state.setSelection('preset-select', 'preset-456');
    state.setSelection('view-select', 'view-789');

    const nextState = useHeaderUiStore.getState();
    expect(Object.keys(nextState.selections)).toHaveLength(3);
    expect(nextState.selections['model-select']).toBe('model-123');
    expect(nextState.selections['preset-select']).toBe('preset-456');
    expect(nextState.selections['view-select']).toBe('view-789');
  });

  it('should update existing selection', () => {
    const state = useHeaderUiStore.getState();
    state.setSelection('model-select', 'model-123');
    state.setSelection('model-select', 'model-456');

    const nextState = useHeaderUiStore.getState();
    expect(nextState.selections['model-select']).toBe('model-456');
    expect(Object.keys(nextState.selections)).toHaveLength(1);
  });
});
