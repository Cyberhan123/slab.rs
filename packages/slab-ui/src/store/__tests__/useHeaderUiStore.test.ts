import { describe, it, expect, beforeEach } from 'vitest';
import './mock-ui-state-storage';
import { migrateHeaderUiState, useHeaderUiStore } from '../useHeaderUiStore';

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

  it('should not clear selection for an empty key', () => {
    useHeaderUiStore.getState().setSelection('model-select', 'model-123');
    useHeaderUiStore.getState().clearSelection('   ');
    expect(useHeaderUiStore.getState().selections['model-select']).toBe('model-123');
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

describe('migrateHeaderUiState', () => {
  it('passes through snapshots that are not selections objects', () => {
    expect(migrateHeaderUiState(null)).toBeNull();
    expect(migrateHeaderUiState(undefined)).toBeUndefined();
    expect(migrateHeaderUiState('string')).toBe('string');
    expect(migrateHeaderUiState({ foo: 1 })).toEqual({ foo: 1 });
  });

  it('leaves selections unchanged when assistant:model is already set', () => {
    const selections = { 'assistant:model': 'gpt-4', other: 'x' };
    expect(migrateHeaderUiState({ selections })).toEqual({ selections });
  });

  it('leaves selections unchanged when there is no chat:model to migrate', () => {
    const selections = { other: 'x' };
    expect(migrateHeaderUiState({ selections })).toEqual({ selections });
  });

  it('copies chat:model to assistant:model and keeps the legacy key', () => {
    const result = migrateHeaderUiState({ selections: { 'chat:model': 'gpt-4' } });
    expect(result.selections).toEqual({
      'chat:model': 'gpt-4',
      'assistant:model': 'gpt-4',
    });
  });
});
