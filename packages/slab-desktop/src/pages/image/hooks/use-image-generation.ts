import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api from '@slab/api';
import type { components } from '@slab/api/v1';
import {
  getImageGeneration,
  listImageGenerations,
  resolveMediaUrl,
  type ImageGenerationTask,
} from '@/lib/media-task-api';
import {
  MAX_POLL_ATTEMPTS,
  POLL_INTERVAL_MS,
  type GeneratedImage,
  type ImageRouteState,
} from '../const';
import { useImageGenerationControls } from './use-image-generation-controls';
import { useImageModelPreparation } from './use-image-model-preparation';

type GenerationPhase = 'idle' | 'polling' | 'fetchingResult';
type TaskResponse = components['schemas']['TaskResponse'];

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => resolve(reader.result as string));
    reader.addEventListener("error", reject);
    reader.readAsDataURL(file);
  });
}

export function useImageGeneration() {
  const { t } = useTranslation();
  const location = useLocation();
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [generationPhase, setGenerationPhase] = useState<GenerationPhase>('idle');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [images, setImages] = useState<GeneratedImage[]>([]);
  const [zoomedImage, setZoomedImage] = useState<string | null>(null);
  const [history, setHistory] = useState<ImageGenerationTask[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [selectedHistoryTask, setSelectedHistoryTask] = useState<ImageGenerationTask | null>(null);
  const [historyDialogOpen, setHistoryDialogOpen] = useState(false);

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

  const generateImagesMutation = api.useMutation('post', '/v1/images/generations');
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel');

  const isPolling = generationPhase === 'polling';
  const isFetchingResult = generationPhase === 'fetchingResult';

  // Cleanup polling state on unmount to prevent memory leaks
  useEffect(() => {
    return () => {
      if (isPolling || isFetchingResult) {
        clearGenerationTask();
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
    icon: PAGE_HEADER_META.image.icon,
    title: t('pages.image.header.title'),
    subtitle:
      mode === 'img2img'
        ? t('pages.image.header.subtitleImg2Img')
        : t('pages.image.header.subtitleTxt2Img'),
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
      groupLabel: t('pages.image.modelPicker.groupLabel'),
      placeholder: t('pages.image.modelPicker.placeholder'),
      loading: catalogLoading,
      disabled: catalogLoading || isBusy || modelOptions.length === 0,
      emptyLabel: t('pages.image.modelPicker.emptyLabel'),
    }),
    [catalogLoading, isBusy, modelOptions, selectedModelId, setSelectedModelId, t],
  );

  usePageHeaderControl(headerModelPicker);

  const clearGenerationTask = useCallback(() => {
    pollAttempts.current = 0;
    setGenerationPhase('idle');
    setTaskId(null);
  }, []);

  const toGeneratedImages = useCallback((task: ImageGenerationTask): GeneratedImage[] => {
    const width = task.width || 512;
    const height = task.height || 512;
    const generationMode = task.mode === 'img2img' ? 'img2img' : 'txt2img';

    return task.image_urls
      .map((url) => resolveMediaUrl(url))
      .filter((url): url is string => typeof url === 'string' && url.length > 0)
      .map((src) => ({
        src,
        prompt: task.prompt,
        width,
        height,
        mode: generationMode,
      }));
  }, []);

  const mergeHistoryTask = useCallback((task: ImageGenerationTask) => {
    setHistory((previous) => {
      const next = [task, ...previous.filter((entry) => entry.task_id !== task.task_id)];
      return next.toSorted((left, right) => Date.parse(right.created_at) - Date.parse(left.created_at));
    });
  }, []);

  const refreshHistory = useCallback(async () => {
    try {
      setHistoryLoading(true);
      setHistoryError(null);
      const items = await listImageGenerations();
      setHistory(items);
    } catch (error) {
      setHistoryError(error instanceof Error ? error.message : String(error));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  const openHistoryDetail = useCallback(async (taskIdToOpen: string) => {
    try {
      const detail = await getImageGeneration(taskIdToOpen);
      setSelectedHistoryTask(detail);
      setHistoryDialogOpen(true);
      mergeHistoryTask(detail);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('pages.image.toast.historyDetailFailed', { message }));
    }
  }, [mergeHistoryTask, t]);

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

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
        toast.error(t('pages.image.error.readImageFileFailed'));
      }
    },
    [t],
  );

  const handleSubmit = useCallback(async () => {
    if (isResolvingModelState) {
      toast.error(t('pages.image.error.modelPresetLoading'));
      return;
    }

    if (!prompt.trim()) {
      toast.error(t('pages.image.error.enterPrompt'));
      return;
    }

    if (mode === 'img2img' && !initImageDataUri) {
      toast.error(t('pages.image.error.uploadInitImage'));
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
          model_id: selectedModelId || undefined,
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
        } as never,
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
    selectedModelId,
    steps,
    strength,
    t,
    widthStr,
  ]);

  useEffect(() => {
    if (!isPolling || !taskId || taskStatusUpdatedAt === 0) {
      return;
    }

    pollAttempts.current += 1;
    if (pollAttempts.current > MAX_POLL_ATTEMPTS) {
      toast.error(t('pages.image.toast.generationTimedOut'));
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
      toast.error(taskStatus.error_msg ?? t('pages.image.error.generationFailed'));
      clearGenerationTask();
      return;
    }

    if (taskStatus.status === 'succeeded') {
      setGenerationPhase('fetchingResult');
    }
  }, [clearGenerationTask, isPolling, taskId, taskStatus, taskStatusUpdatedAt, t]);

  useEffect(() => {
    if (!isPolling || !taskId || !taskStatusError) {
      return;
    }

    const message = taskStatusError instanceof Error ? taskStatusError.message : String(taskStatusError);
    toast.error(t('pages.image.toast.pollingError', { message }));
    clearGenerationTask();
  }, [clearGenerationTask, isPolling, taskId, taskStatusError, t]);

  useEffect(() => {
    if (!isFetchingResult || !taskId) {
      return;
    }

    let cancelled = false;

    const loadResult = async () => {
      try {
        const detail = await getImageGeneration(taskId);
        if (cancelled) {
          return;
        }

        const generated = toGeneratedImages(detail);
        mergeHistoryTask(detail);
        setSelectedHistoryTask(detail);
        setImages((previous) => [...generated, ...previous]);
        setHistoryDialogOpen(true);
        toast.success(t('pages.image.toast.generated', { count: generated.length }));
      } catch (error) {
        if (cancelled) {
          return;
        }

        const message = error instanceof Error ? error.message : String(error);
        toast.error(t('pages.image.toast.resultFetchFailed', { message }));
      } finally {
        if (!cancelled) {
          clearGenerationTask();
          void refreshHistory();
        }
      }
    };

    void loadResult();

    return () => {
      cancelled = true;
    };
  }, [clearGenerationTask, isFetchingResult, mergeHistoryTask, refreshHistory, t, taskId, toGeneratedImages]);

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
    anchor.href = resolveMediaUrl(src) ?? src;
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
    history,
    historyDialogOpen,
    historyError,
    historyLoading,
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
    selectedHistoryTask,
    selectedModelId,
    setHistoryDialogOpen,
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
    setSelectedHistoryTask,
    setZoomedImage,
    steps,
    strength,
    widthStr,
    zoomedImage,
    openHistoryDetail,
  };
}
