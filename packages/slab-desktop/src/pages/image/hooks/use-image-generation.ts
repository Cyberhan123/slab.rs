import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { toast } from 'sonner';

import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';
import {
  MAX_POLL_ATTEMPTS,
  POLL_INTERVAL_MS,
  type GeneratedImage,
  type ImageRouteState,
} from '../const';
import { useImageGenerationControls } from './use-image-generation-controls';
import { useImageModelPreparation } from './use-image-model-preparation';

type GenerationPhase = 'idle' | 'polling' | 'fetchingResult';
type ImageGenerationRequest = components['schemas']['ImageGenerationRequest'];
type OperationAcceptedResponse = components['schemas']['OperationAcceptedResponse'];
type TaskResponse = components['schemas']['TaskResponse'];
type TaskResultPayload = components['schemas']['TaskResultPayload'];

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
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [generationPhase, setGenerationPhase] = useState<GenerationPhase>('idle');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [images, setImages] = useState<GeneratedImage[]>([]);
  const [zoomedImage, setZoomedImage] = useState<string | null>(null);

  const initImageInputRef = useRef<HTMLInputElement>(null);
  const pollAttempts = useRef(0);

  const {
    catalogLoading,
    isPreparingModel,
    modelOptions,
    prepareSelectedModel,
    selectedModelId,
    setSelectedModelId,
  } = useImageModelPreparation();
  const {
    activeDimensionPreset,
    advancedOpen,
    cfgScale,
    clipSkip,
    eta,
    guidance,
    handleDimensionPreset,
    heightStr,
    isResolvingModelState,
    mode,
    numImages,
    parsedHeight,
    parsedWidth,
    sampleMethod,
    scheduler,
    seed,
    setAdvancedOpen,
    setCfgScale,
    setClipSkip,
    setEta,
    setGuidance,
    setHeightStr,
    setMode,
    setNumImages,
    setSampleMethod,
    setScheduler,
    setSeed,
    setSteps,
    setStrength,
    setWidthStr,
    steps,
    strength,
    widthStr,
  } = useImageGenerationControls(selectedModelId);

  const generateImagesMutation = api.useMutation('post', '/v1/images/generations') as unknown as {
    mutateAsync: (options: { body: ImageGenerationRequest }) => Promise<OperationAcceptedResponse>;
  };
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel');

  const isPolling = generationPhase === 'polling';
  const isFetchingResult = generationPhase === 'fetchingResult';

  const {
    data: taskStatus,
    error: taskStatusError,
    dataUpdatedAt: taskStatusUpdatedAt,
  } = api.useQuery(
    'get',
    '/v1/tasks/{id}',
    {
      params: {
        path: {
          id: taskId ?? '',
        },
      },
    },
    {
      enabled: isPolling && Boolean(taskId),
      refetchInterval: isPolling && taskId ? POLL_INTERVAL_MS : false,
      refetchIntervalInBackground: true,
      retry: false,
    },
  ) as {
    data: TaskResponse | undefined;
    error: unknown;
    dataUpdatedAt: number;
  };

  const {
    data: taskResult,
    error: taskResultError,
    dataUpdatedAt: taskResultUpdatedAt,
  } = api.useQuery(
    'get',
    '/v1/tasks/{id}/result',
    {
      params: {
        path: {
          id: taskId ?? '',
        },
      },
    },
    {
      enabled: isFetchingResult && Boolean(taskId),
      retry: false,
    },
  ) as {
    data: TaskResultPayload | undefined;
    error: unknown;
    dataUpdatedAt: number;
  };

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

  const isGenerating = isSubmitting || generationPhase !== 'idle';
  const isBusy = isGenerating || isPreparingModel || isResolvingModelState;
  const headerModelPicker = useMemo(
    () => ({
      type: 'select' as const,
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

  usePageHeaderControl(headerModelPicker);

  const clearGenerationTask = useCallback(() => {
    pollAttempts.current = 0;
    setGenerationPhase('idle');
    setTaskId(null);
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
    if (isResolvingModelState) {
      toast.error('Model preset is still loading');
      return;
    }

    if (!prompt.trim()) {
      toast.error('Please enter a prompt');
      return;
    }

    if (mode === 'img2img' && !initImageDataUri) {
      toast.error('Please upload an init image for img2img mode');
      return;
    }

    try {
      setIsSubmitting(true);
      clearGenerationTask();

      const modelPath = await prepareSelectedModel();
      const width = Number.parseInt(widthStr, 10) || 512;
      const height = Number.parseInt(heightStr, 10) || 512;

      const { operation_id } = await generateImagesMutation.mutateAsync({
        body: {
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
        },
      });

      setTaskId(operation_id);
      setGenerationPhase('polling');
      pollAttempts.current = 0;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [
    cfgScale,
    clearGenerationTask,
    clipSkip,
    eta,
    generateImagesMutation,
    guidance,
    heightStr,
    initImageDataUri,
    isResolvingModelState,
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
    if (!isPolling || !taskId || taskStatusUpdatedAt === 0) {
      return;
    }

    pollAttempts.current += 1;
    if (pollAttempts.current > MAX_POLL_ATTEMPTS) {
      toast.error('Generation timed out');
      clearGenerationTask();
      return;
    }

    if (!taskStatus) {
      return;
    }

    if (
      taskStatus.status === 'failed' ||
      taskStatus.status === 'cancelled' ||
      taskStatus.status === 'interrupted'
    ) {
      toast.error(taskStatus.error_msg ?? 'Image generation failed');
      clearGenerationTask();
      return;
    }

    if (taskStatus.status === 'succeeded') {
      setGenerationPhase('fetchingResult');
    }
  }, [clearGenerationTask, isPolling, taskId, taskStatus, taskStatusUpdatedAt]);

  useEffect(() => {
    if (!isPolling || !taskId || !taskStatusError) {
      return;
    }

    const message = taskStatusError instanceof Error ? taskStatusError.message : String(taskStatusError);
    toast.error(`Polling error: ${message}`);
    clearGenerationTask();
  }, [clearGenerationTask, isPolling, taskId, taskStatusError]);

  useEffect(() => {
    if (!isFetchingResult || !taskId || taskResultUpdatedAt === 0 || !taskResult) {
      return;
    }

    const srcs = taskResult.images ?? (taskResult.image ? [taskResult.image] : []);
    const width = Number.parseInt(widthStr, 10) || 512;
    const height = Number.parseInt(heightStr, 10) || 512;

    const generated: GeneratedImage[] = srcs
      .filter((src): src is string => typeof src === 'string' && src.length > 0)
      .map((src) => ({
        src,
        prompt,
        width,
        height,
        mode,
      }));

    setImages((previous) => [...generated, ...previous]);
    toast.success(`Generated ${generated.length} image${generated.length !== 1 ? 's' : ''}!`);
    clearGenerationTask();
  }, [
    clearGenerationTask,
    heightStr,
    isFetchingResult,
    mode,
    prompt,
    taskId,
    taskResult,
    taskResultUpdatedAt,
    widthStr,
  ]);

  useEffect(() => {
    if (!isFetchingResult || !taskId || !taskResultError) {
      return;
    }

    const message = taskResultError instanceof Error ? taskResultError.message : String(taskResultError);
    toast.error(`Failed to fetch generation result: ${message}`);
    clearGenerationTask();
  }, [clearGenerationTask, isFetchingResult, taskId, taskResultError]);

  const handleCancel = useCallback(async () => {
    if (taskId) {
      try {
        await cancelTaskMutation.mutateAsync({
          params: {
            path: { id: taskId },
          },
        });
      } catch (error) {
        console.error('Failed to cancel task', error);
      }
    }

    clearGenerationTask();
  }, [cancelTaskMutation, clearGenerationTask, taskId]);

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
    isResolvingModelState,
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
