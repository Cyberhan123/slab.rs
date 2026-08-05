import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from 'vitest-browser-react';

vi.mock('@/store/ui-state-storage', () => ({
  createUiStateStorage: () => ({
    getItem: () => Promise.resolve(null),
    setItem: () => Promise.resolve(),
    removeItem: () => Promise.resolve(),
  }),
}));
vi.mock('@/lib/model-config', () => ({
  useModelConfigDocumentQuery: vi.fn<() => unknown>(() => ({ data: undefined, error: null })),
  getModelConfigFieldValue: () => undefined,
}));

import { useImageUiStore } from '@/store/useImageUiStore';
import { useImageGenerationControls } from '../use-image-generation-controls';
import { createDefaultImageGenerationControls } from '../../lib/image-generation-controls';

describe('useImageGenerationControls', () => {
  beforeEach(() => {
    useImageUiStore.setState({ hasHydrated: true, modelControls: {} });
  });

  it('uses default controls and the square preset when no model is selected', async () => {
    const { result } = await renderHook(() => useImageGenerationControls(''));

    expect(result.current.mode).toBe('txt2img');
    expect(result.current.activeDimensionPreset).toBe('1:1');
  });

  it('rehydrates persisted controls for the selected model', async () => {
    useImageUiStore.setState({
      hasHydrated: true,
      modelControls: {
        m1: {
          ...createDefaultImageGenerationControls(),
          mode: 'img2img',
          widthStr: '768',
          heightStr: '576',
        },
      },
    });

    const { result } = await renderHook(() => useImageGenerationControls('m1'));

    await vi.waitFor(() => expect(result.current.mode).toBe('img2img'));
    expect(result.current.activeDimensionPreset).toBe('4:3');
  });

  it('updates controls through the setter surface and resolves the active preset', async () => {
    const { result, act } = await renderHook(() => useImageGenerationControls(''));

    await act(() => {
      result.current.setMode('img2img');
    });
    expect(result.current.mode).toBe('img2img');

    await act(() => {
      result.current.setWidthStr('1024');
    });
    await act(() => {
      result.current.setHeightStr('576');
    });
    expect(result.current.activeDimensionPreset).toBe('16:9');
  });

  it('persists control changes back into the ui store once resolved', async () => {
    useImageUiStore.setState({
      hasHydrated: true,
      modelControls: { m1: { ...createDefaultImageGenerationControls(), steps: 20 } },
    });

    const { result, act } = await renderHook(() => useImageGenerationControls('m1'));

    await vi.waitFor(() => expect(result.current.steps).toBe(20));

    await act(() => {
      result.current.setSteps(30);
    });

    await vi.waitFor(() => {
      expect(useImageUiStore.getState().modelControls.m1?.steps).toBe(30);
    });
  });
});
