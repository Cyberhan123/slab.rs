import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { toast } from 'sonner';

import { usePageHeader, usePageHeaderModelPicker } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import {
  API_BASE_URL,
  DIMENSION_PRESETS,
  MAX_POLL_ATTEMPTS,
  POLL_INTERVAL_MS,
  type GeneratedImage,
  type ImageRouteState,
  type TaskResult,
} from '../const';
import { useImageModelPreparation } from './use-image-model-preparation';

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export function useImageGeneration() {
  const location = useLocation();
  const [mode, setMode] = useState<'txt2img' | 'img2img'>('txt2img');
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [widthStr, setWidthStr] = useState('512');
  const [heightStr, setHeightStr] = useState('512');
  const [numImages, setNumImages] = useState(1);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [cfgScale, setCfgScale] = useState(7.0);
  const [guidance, setGuidance] = useState(3.5);
  const [steps, setSteps] = useState(20);
  const [seed, setSeed] = useState(-1);
  const [sampleMethod, setSampleMethod] = useState('auto');
  const [scheduler, setScheduler] = useState('auto');
  const [clipSkip, setClipSkip] = useState(0);
  const [eta, setEta] = useState(0);
  const [strength, setStrength] = useState(0.75);
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [images, setImages] = useState<GeneratedImage[]>([]);
  const [zoomedImage, setZoomedImage] = useState<string | null>(null);

  const initImageInputRef = useRef<HTMLInputElement>(null);
  const pollAttempts = useRef(0);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef(false);

  const {
    catalogLoading,
    isPreparingModel,
    modelOptions,
    prepareSelectedModel,
    selectedModelId,
    setSelectedModelId,
  } = useImageModelPreparation();

  const getPrefilledPrompt = useCallback(() => {
    const statePrompt =
      typeof (location.state as ImageRouteState | null)?.prompt === 'string'
        ? (location.state as ImageRouteState).prompt
        : null;

    const search = new URLSearchParams(location.search);
    const queryPrompt = search.get('prompt') ?? search.get('q');

    return (statePrompt ?? queryPrompt ?? '').trim();
  }, [location.search, location.state]);

  useEffect(() => {
    const prefilled = getPrefilledPrompt();
    if (prefilled) {
      setPrompt(prefilled);
    }
  }, [getPrefilledPrompt, location.key]);

  usePageHeader({
    ...PAGE_HEADER_META.image,
    subtitle:
      mode === 'img2img'
        ? 'Refine an input image with diffusion controls'
        : 'Generate images from text prompts',
  });

  const isGenerating = isSubmitting || isPolling;
  const isBusy = isGenerating || isPreparingModel;
  const headerModelPicker = useMemo(
    () => ({
      value: selectedModelId,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.label,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: 'Image Models',
      placeholder: 'Select model',
      loading: catalogLoading,
      disabled: catalogLoading || isBusy || modelOptions.length === 0,
      emptyLabel: 'No diffusion models',
    }),
    [catalogLoading, isBusy, modelOptions, selectedModelId, setSelectedModelId],
  );
  const parsedWidth = Number.parseInt(widthStr, 10) || 512;
  const parsedHeight = Number.parseInt(heightStr, 10) || 512;
  const activeDimensionPreset =
    DIMENSION_PRESETS.find(
      (preset) => preset.width === parsedWidth && preset.height === parsedHeight,
    )?.label ?? null;

  usePageHeaderModelPicker(headerModelPicker);

  const handleDimensionPreset = useCallback((width: number, height: number) => {
    setWidthStr(String(width));
    setHeightStr(String(height));
  }, []);

  const handleInitImageChange = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (!file) {
        return;
      }
      try {
        const dataUri = await fileToDataUri(file);
        setInitImageDataUri(dataUri);
      } catch {
        toast.error('Failed to read image file');
      }
    },
    [],
  );

  const handleSubmit = useCallback(async () => {
    if (!prompt.trim()) {
      toast.error('Please enter a prompt');
      return;
    }

    if (mode === 'img2img' && !initImageDataUri) {
      toast.error('Please upload an init image for img2img mode');
      return;
    }

    abortRef.current = false;

    try {
      const modelPath = await prepareSelectedModel();
      setIsSubmitting(true);

      const width = Number.parseInt(widthStr, 10) || 512;
      const height = Number.parseInt(heightStr, 10) || 512;

      const response = await fetch(`${API_BASE_URL}/v1/images/generations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: modelPath,
          prompt,
          negative_prompt: negativePrompt || undefined,
          n: numImages,
          width,
          height,
          cfg_scale: cfgScale,
          guidance,
          steps,
          seed: seed < 0 ? Math.floor(Math.random() * 2147483647) : seed,
          sample_method: sampleMethod === 'auto' ? undefined : sampleMethod,
          scheduler: scheduler === 'auto' ? undefined : scheduler,
          clip_skip: clipSkip || undefined,
          eta: eta !== 0 ? eta : undefined,
          strength: mode === 'img2img' ? strength : undefined,
          init_image: mode === 'img2img' ? initImageDataUri : undefined,
          mode,
        }),
      });

      if (!response.ok) {
        const detail = await response.text();
        throw new Error(`HTTP ${response.status}: ${detail || 'generation failed'}`);
      }

      const { operation_id } = (await response.json()) as { operation_id: string };
      setTaskId(operation_id);
      setIsPolling(true);
      pollAttempts.current = 0;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [
    cfgScale,
    clipSkip,
    eta,
    guidance,
    heightStr,
    initImageDataUri,
    mode,
    negativePrompt,
    numImages,
    prepareSelectedModel,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    steps,
    strength,
    widthStr,
  ]);

  useEffect(() => {
    if (!isPolling || !taskId) {
      return;
    }

    const poll = async () => {
      if (abortRef.current) {
        setIsPolling(false);
        return;
      }

      pollAttempts.current += 1;
      if (pollAttempts.current > MAX_POLL_ATTEMPTS) {
        toast.error('Generation timed out');
        setIsPolling(false);
        setTaskId(null);
        return;
      }

      try {
        const statusRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}`);
        if (!statusRes.ok) {
          throw new Error(`status ${statusRes.status}`);
        }
        const status = (await statusRes.json()) as { status: string };

        if (status.status === 'failed') {
          toast.error('Image generation failed');
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        if (status.status === 'succeeded') {
          const resultRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/result`);
          if (!resultRes.ok) {
            throw new Error(`result ${resultRes.status}`);
          }
          const result = (await resultRes.json()) as TaskResult;

          const srcs = result.images ?? (result.image ? [result.image] : []);
          const width = Number.parseInt(widthStr, 10) || 512;
          const height = Number.parseInt(heightStr, 10) || 512;

          const generated: GeneratedImage[] = srcs.map((src) => ({
            src,
            prompt,
            width,
            height,
            mode,
          }));

          setImages((previous) => [...generated, ...previous]);
          toast.success(
            `Generated ${generated.length} image${generated.length !== 1 ? 's' : ''}!`,
          );
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        pollTimer.current = setTimeout(poll, POLL_INTERVAL_MS);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        toast.error(`Polling error: ${message}`);
        setIsPolling(false);
        setTaskId(null);
      }
    };

    pollTimer.current = setTimeout(poll, POLL_INTERVAL_MS);
    return () => {
      if (pollTimer.current) {
        clearTimeout(pollTimer.current);
      }
    };
  }, [heightStr, isPolling, mode, prompt, taskId, widthStr]);

  const handleCancel = useCallback(async () => {
    abortRef.current = true;
    if (pollTimer.current) {
      clearTimeout(pollTimer.current);
    }
    if (taskId) {
      try {
        await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/cancel`, { method: 'POST' });
      } catch (error) {
        console.error('Failed to cancel task', error);
      }
    }
    setIsPolling(false);
    setTaskId(null);
  }, [taskId]);

  const handleDownload = useCallback((src: string, index: number) => {
    const anchor = document.createElement('a');
    anchor.href = src;
    anchor.download = `generated-${index + 1}.png`;
    anchor.click();
  }, []);

  return {
    activeDimensionPreset,
    advancedOpen,
    cfgScale,
    clipSkip,
    eta,
    guidance,
    handleCancel,
    handleDimensionPreset,
    handleDownload,
    handleInitImageChange,
    handleSubmit,
    heightStr,
    images,
    initImageDataUri,
    initImageInputRef,
    isBusy,
    isGenerating,
    isPreparingModel,
    mode,
    negativePrompt,
    numImages,
    parsedHeight,
    parsedWidth,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    selectedModelId,
    setAdvancedOpen,
    setCfgScale,
    setClipSkip,
    setEta,
    setGuidance,
    setHeightStr,
    setInitImageDataUri,
    setMode,
    setNegativePrompt,
    setNumImages,
    setPrompt,
    setSampleMethod,
    setScheduler,
    setSeed,
    setSteps,
    setStrength,
    setWidthStr,
    setZoomedImage,
    steps,
    strength,
    widthStr,
    zoomedImage,
  };
}
