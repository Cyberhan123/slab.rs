import { useCallback, useEffect, useMemo, useState } from 'react';

import { fetchModelConfigDocument } from '@/lib/model-config';
import { useImageUiStore } from '@/store/useImageUiStore';
import { DIMENSION_PRESETS } from '../const';
import {
  areImageGenerationControlsEqual,
  buildImageGenerationControlsFromModelConfig,
  createDefaultImageGenerationControls,
  normalizeImageGenerationControls,
  type ImageGenerationControls,
  type ImageGenerationMode,
} from '../lib/image-generation-controls';

export function useImageGenerationControls(selectedModelId: string) {
  const hasHydrated = useImageUiStore((state) => state.hasHydrated);
  const modelControls = useImageUiStore((state) => state.modelControls);
  const setModelControls = useImageUiStore((state) => state.setModelControls);

  const [controls, setControls] = useState<ImageGenerationControls>(() =>
    createDefaultImageGenerationControls(),
  );
  const [resolvedModelId, setResolvedModelId] = useState<string | null>(null);

  const persistedControls = selectedModelId ? modelControls[selectedModelId] : undefined;
  const isResolvingModelState = Boolean(selectedModelId) && (!hasHydrated || resolvedModelId !== selectedModelId);

  useEffect(() => {
    if (!hasHydrated) {
      return;
    }

    if (!selectedModelId) {
      setControls(createDefaultImageGenerationControls());
      setResolvedModelId(null);
      return;
    }

    if (resolvedModelId === selectedModelId) {
      return;
    }

    if (persistedControls) {
      setControls(normalizeImageGenerationControls(persistedControls));
      setResolvedModelId(selectedModelId);
      return;
    }

    setControls(createDefaultImageGenerationControls());

    let disposed = false;

    void fetchModelConfigDocument(selectedModelId)
      .then((document) => {
        if (disposed) {
          return;
        }

        setControls(buildImageGenerationControlsFromModelConfig(document));
        setResolvedModelId(selectedModelId);
      })
      .catch((error) => {
        if (disposed) {
          return;
        }

        console.warn(`Failed to load image preset defaults for model '${selectedModelId}'.`, error);
        setControls(createDefaultImageGenerationControls());
        setResolvedModelId(selectedModelId);
      });

    return () => {
      disposed = true;
    };
  }, [hasHydrated, persistedControls, resolvedModelId, selectedModelId]);

  useEffect(() => {
    if (!hasHydrated || !selectedModelId || resolvedModelId !== selectedModelId) {
      return;
    }

    if (areImageGenerationControlsEqual(persistedControls, controls)) {
      return;
    }

    setModelControls(selectedModelId, controls);
  }, [
    controls,
    hasHydrated,
    persistedControls,
    resolvedModelId,
    selectedModelId,
    setModelControls,
  ]);

  const updateControls = useCallback((patch: Partial<ImageGenerationControls>) => {
    setControls((current) => ({ ...current, ...patch }));
  }, []);

  const parsedWidth = Number.parseInt(controls.widthStr, 10) || 512;
  const parsedHeight = Number.parseInt(controls.heightStr, 10) || 512;

  const activeDimensionPreset = useMemo(
    () =>
      DIMENSION_PRESETS.find(
        (preset) => preset.width === parsedWidth && preset.height === parsedHeight,
      )?.label ?? null,
    [parsedHeight, parsedWidth],
  );

  return {
    ...controls,
    activeDimensionPreset,
    handleDimensionPreset: (width: number, height: number) =>
      updateControls({
        widthStr: String(width),
        heightStr: String(height),
      }),
    isResolvingModelState,
    parsedHeight,
    parsedWidth,
    setAdvancedOpen: (advancedOpen: boolean) => updateControls({ advancedOpen }),
    setCfgScale: (cfgScale: number) => updateControls({ cfgScale }),
    setClipSkip: (clipSkip: number) => updateControls({ clipSkip }),
    setEta: (eta: number) => updateControls({ eta }),
    setGuidance: (guidance: number) => updateControls({ guidance }),
    setHeightStr: (heightStr: string) => updateControls({ heightStr }),
    setMode: (mode: ImageGenerationMode) => updateControls({ mode }),
    setNumImages: (numImages: number) => updateControls({ numImages }),
    setSampleMethod: (sampleMethod: string) => updateControls({ sampleMethod }),
    setScheduler: (scheduler: string) => updateControls({ scheduler }),
    setSeed: (seed: number) => updateControls({ seed }),
    setSteps: (steps: number) => updateControls({ steps }),
    setStrength: (strength: number) => updateControls({ strength }),
    setWidthStr: (widthStr: string) => updateControls({ widthStr }),
  };
}
