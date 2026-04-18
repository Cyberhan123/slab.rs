import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useAudioUiStore } from '../useAudioUiStore';

// Mock the UI state storage
vi.mock('../ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

describe('useAudioUiStore', () => {
  beforeEach(() => {
    useAudioUiStore.setState({
      modelControlOverrides: {},
      hasHydrated: false,
    });
  });

  it('should have initial state', () => {
    const state = useAudioUiStore.getState();
    expect(state.modelControlOverrides).toEqual({});
    expect(state.hasHydrated).toBe(false);
  });

  it('should set model control overrides', () => {
    const overrides = { decodeTemperature: '0.8', language: 'en' };
    useAudioUiStore.getState().setModelControlOverrides('model-123', overrides);
    expect(useAudioUiStore.getState().modelControlOverrides['model-123']).toEqual(overrides);
  });

  it('should trim whitespace from model ID', () => {
    const overrides = { decodeTemperature: '0.8' };
    useAudioUiStore.getState().setModelControlOverrides('  model-123  ', overrides);
    expect(useAudioUiStore.getState().modelControlOverrides['model-123']).toEqual(overrides);
  });

  it('should not set overrides for empty model ID', () => {
    const overrides = { decodeTemperature: '0.8' };
    useAudioUiStore.getState().setModelControlOverrides('', overrides);
    expect(useAudioUiStore.getState().modelControlOverrides).toEqual({});
  });

  it('should remove overrides when empty object is provided', () => {
    const overrides = { decodeTemperature: '0.8' };
    const state = useAudioUiStore.getState();
    state.setModelControlOverrides('model-123', overrides);
    state.setModelControlOverrides('model-123', {});
    expect(useAudioUiStore.getState().modelControlOverrides['model-123']).toBeUndefined();
  });

  it('should clear model control overrides', () => {
    const overrides = { decodeTemperature: '0.8' };
    const state = useAudioUiStore.getState();
    state.setModelControlOverrides('model-123', overrides);
    state.clearModelControlOverrides('model-123');
    expect(useAudioUiStore.getState().modelControlOverrides['model-123']).toBeUndefined();
  });

  it('should handle clearing non-existent overrides', () => {
    useAudioUiStore.getState().clearModelControlOverrides('non-existent');
    expect(useAudioUiStore.getState().modelControlOverrides).toEqual({});
  });

  it('should set hasHydrated state', () => {
    useAudioUiStore.getState().setHasHydrated(true);
    expect(useAudioUiStore.getState().hasHydrated).toBe(true);
  });

  it('should maintain overrides for multiple models', () => {
    const state = useAudioUiStore.getState();
    state.setModelControlOverrides('model-1', { decodeTemperature: '0.8' });
    state.setModelControlOverrides('model-2', { language: 'en' });
    state.setModelControlOverrides('model-3', { decodeTemperature: '0.7', language: 'zh' });

    const nextState = useAudioUiStore.getState();
    expect(Object.keys(nextState.modelControlOverrides)).toHaveLength(3);
    expect(nextState.modelControlOverrides['model-1']).toEqual({ decodeTemperature: '0.8' });
    expect(nextState.modelControlOverrides['model-2']).toEqual({ language: 'en' });
    expect(nextState.modelControlOverrides['model-3']).toEqual({
      decodeTemperature: '0.7',
      language: 'zh',
    });
  });

  it('should update existing overrides', () => {
    const state = useAudioUiStore.getState();
    state.setModelControlOverrides('model-123', { decodeTemperature: '0.8' });
    state.setModelControlOverrides('model-123', { decodeTemperature: '0.9', language: 'ja' });

    const nextState = useAudioUiStore.getState();
    expect(nextState.modelControlOverrides['model-123']).toEqual({
      decodeTemperature: '0.9',
      language: 'ja',
    });
    expect(Object.keys(nextState.modelControlOverrides)).toHaveLength(1);
  });
});
