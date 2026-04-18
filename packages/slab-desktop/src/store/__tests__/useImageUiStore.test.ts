import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  createDefaultImageGenerationControls,
  type ImageGenerationControls,
} from '@/pages/image/lib/image-generation-controls';
import { useImageUiStore } from '../useImageUiStore';

// Mock the UI state storage
vi.mock('../ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: vi.fn<() => Promise<null>>(async () => null),
    setItem: vi.fn<() => Promise<void>>(async () => {}),
    removeItem: vi.fn<() => Promise<void>>(async () => {}),
  }),
}));

function makeControls(
  overrides: Partial<ImageGenerationControls> = {},
): ImageGenerationControls {
  return {
    ...createDefaultImageGenerationControls(),
    ...overrides,
  };
}

describe('useImageUiStore', () => {
  beforeEach(() => {
    useImageUiStore.setState({
      modelControls: {},
      hasHydrated: false,
    });
  });

  it('should have initial state', () => {
    const state = useImageUiStore.getState();
    expect(state.modelControls).toEqual({});
    expect(state.hasHydrated).toBe(false);
  });

  it('should set model controls', () => {
    const controls = makeControls({ widthStr: '512', heightStr: '512', steps: 20 });
    useImageUiStore.getState().setModelControls('model-123', controls);
    expect(useImageUiStore.getState().modelControls['model-123']).toEqual(controls);
  });

  it('should trim whitespace from model ID', () => {
    const controls = makeControls({ widthStr: '512', heightStr: '512' });
    useImageUiStore.getState().setModelControls('  model-123  ', controls);
    expect(useImageUiStore.getState().modelControls['model-123']).toEqual(controls);
  });

  it('should not set controls for empty model ID', () => {
    const controls = makeControls({ widthStr: '512', heightStr: '512' });
    useImageUiStore.getState().setModelControls('', controls);
    expect(useImageUiStore.getState().modelControls).toEqual({});
  });

  it('should set hasHydrated state', () => {
    useImageUiStore.getState().setHasHydrated(true);
    expect(useImageUiStore.getState().hasHydrated).toBe(true);
  });

  it('should maintain controls for multiple models', () => {
    const state = useImageUiStore.getState();
    state.setModelControls('model-1', makeControls({ widthStr: '512', heightStr: '512', steps: 20 }));
    state.setModelControls('model-2', makeControls({ widthStr: '768', heightStr: '768', steps: 30 }));
    state.setModelControls('model-3', makeControls({ widthStr: '1024', heightStr: '1024', steps: 40 }));

    const nextState = useImageUiStore.getState();
    expect(Object.keys(nextState.modelControls)).toHaveLength(3);
    expect(nextState.modelControls['model-1']).toEqual(
      makeControls({ widthStr: '512', heightStr: '512', steps: 20 }),
    );
    expect(nextState.modelControls['model-2']).toEqual(
      makeControls({ widthStr: '768', heightStr: '768', steps: 30 }),
    );
    expect(nextState.modelControls['model-3']).toEqual(
      makeControls({ widthStr: '1024', heightStr: '1024', steps: 40 }),
    );
  });

  it('should update existing controls', () => {
    const state = useImageUiStore.getState();
    state.setModelControls('model-123', makeControls({ widthStr: '512', heightStr: '512', steps: 20 }));
    state.setModelControls('model-123', makeControls({ widthStr: '768', heightStr: '768', steps: 30 }));

    const nextState = useImageUiStore.getState();
    expect(nextState.modelControls['model-123']).toEqual(
      makeControls({ widthStr: '768', heightStr: '768', steps: 30 }),
    );
    expect(Object.keys(nextState.modelControls)).toHaveLength(1);
  });

  it('should handle complex control objects', () => {
    const complexControls = makeControls({
      widthStr: '512',
      heightStr: '512',
      steps: 20,
      cfgScale: 7.5,
      guidance: 3.5,
      seed: -1,
      sampleMethod: 'euler',
      scheduler: 'discrete',
    });
    useImageUiStore.getState().setModelControls('model-123', complexControls);
    expect(useImageUiStore.getState().modelControls['model-123']).toEqual(complexControls);
  });
});
