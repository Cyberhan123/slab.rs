import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import api from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';
import {
  getVideoGeneration,
  listVideoGenerations,
  resolveMediaUrl,
  type VideoGenerationTask,
} from '@/lib/media-task-api';
import { toCatalogModelList } from '@/lib/api/models';
import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import { HEADER_SELECT_KEYS } from '@/layouts/header-controls';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import {
  MAX_POLL_ATTEMPTS,
  POLL_INTERVAL_MS,
  type ModelOption,
} from '../const';

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

export function useVideoGeneration() {
  const { t } = useTranslation();
  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [widthStr, setWidthStr] = useState('512');
  const [heightStr, setHeightStr] = useState('512');
  const [frames, setFrames] = useState(16);
  const [fps, setFps] = useState(8);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [cfgScale, setCfgScale] = useState(7);
  const [guidance, setGuidance] = useState(3.5);
  const [steps, setSteps] = useState(20);
  const [seed, setSeed] = useState(-1);
  const [sampleMethod, setSampleMethod] = useState('auto');
  const [scheduler, setScheduler] = useState('auto');
  const [strength, setStrength] = useState(0.75);
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [generationPhase, setGenerationPhase] = useState<GenerationPhase>('idle');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [videoPath, setVideoPath] = useState<string | null>(null);
  const [immersivePreview, setImmersivePreview] = useState(false);
  const [history, setHistory] = useState<VideoGenerationTask[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [selectedHistoryTask, setSelectedHistoryTask] = useState<VideoGenerationTask | null>(null);
  const [historyDialogOpen, setHistoryDialogOpen] = useState(false);

  const initImageInputRef = useRef<HTMLInputElement>(null);
  const pollAttempts = useRef(0);

  usePageHeader({
    icon: PAGE_HEADER_META.video.icon,
    title: t('pages.video.header.title'),
    subtitle: t('pages.video.header.subtitle'),
  });

  const { data: catalogModels, isLoading: catalogLoading } = api.useQuery(
    'get',
    '/v1/models',
    {
      params: {
        query: {
          capability: 'video_generation',
        },
      },
    },
  );
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.videoModel,
    options: modelOptions.map((model) => ({
      id: model.id,
      disabled: !model.downloaded,
    })),
    isLoading: catalogLoading,
  });

  useEffect(() => {
    const diffusionModels = toCatalogModelList(catalogModels)
      .filter((model) => model.kind === 'local')
      .map<ModelOption>((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.local_path),
        local_path: model.local_path ?? null,
      }));

    setModelOptions(diffusionModels);
  }, [catalogModels]);

  const selectedModel = useMemo(
    () => modelOptions.find((model) => model.id === selectedModelId),
    [modelOptions, selectedModelId],
  );
  const generateVideoMutation = api.useMutation('post', '/v1/video/generations');
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
  const isGenerating = isSubmitting || generationPhase !== 'idle';
  const headerModelPicker = useMemo(
    () => ({
      type: 'select' as const,
      value: selectedModelId,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.downloaded
          ? model.label
          : t('pages.video.modelPicker.optionDownloadInHub', { model: model.label }),
        disabled: !model.downloaded,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: t('pages.video.modelPicker.groupLabel'),
      placeholder: t('pages.video.modelPicker.placeholder'),
      loading: catalogLoading,
      disabled: catalogLoading || isGenerating || !modelOptions.some((model) => model.downloaded),
      emptyLabel: t('pages.video.modelPicker.emptyLabel'),
    }),
    [catalogLoading, isGenerating, modelOptions, selectedModelId, setSelectedModelId, t],
  );

  usePageHeaderControl(headerModelPicker);

  const clearGenerationTask = useCallback(() => {
    pollAttempts.current = 0;
    setGenerationPhase('idle');
    setTaskId(null);
  }, []);

  const mergeHistoryTask = useCallback((task: VideoGenerationTask) => {
    setHistory((previous) => {
      const next = [task, ...previous.filter((entry) => entry.task_id !== task.task_id)];
      return next.toSorted((left, right) => Date.parse(right.created_at) - Date.parse(left.created_at));
    });
  }, []);

  const refreshHistory = useCallback(async () => {
    try {
      setHistoryLoading(true);
      setHistoryError(null);
      const items = await listVideoGenerations();
      setHistory(items);
    } catch (error) {
      setHistoryError(error instanceof Error ? error.message : String(error));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  const openHistoryDetail = useCallback(async (taskIdToOpen: string) => {
    try {
      const detail = await getVideoGeneration(taskIdToOpen);
      setSelectedHistoryTask(detail);
      setHistoryDialogOpen(true);
      mergeHistoryTask(detail);
      setVideoPath(resolveMediaUrl(detail.video_url));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('pages.video.toast.historyDetailFailed', { message }));
    }
  }, [mergeHistoryTask, t]);

  useEffect(() => {
    void refreshHistory();
  }, [refreshHistory]);

  const loadInitImageFile = useCallback(async (file: File) => {
    if (!file.type.startsWith('image/')) {
      toast.error(t('pages.video.error.chooseImageFile'));
      return;
    }
    try {
      const dataUri = await fileToDataUri(file);
      setInitImageDataUri(dataUri);
    } catch {
      toast.error(t('pages.video.error.readImageFileFailed'));
    }
  }, [t]);

  const handleInitImageChange = useCallback(
    async (event: React.ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (!file) {
        return;
      }
      await loadInitImageFile(file);
    },
    [loadInitImageFile],
  );

  const handleInitImageDrop = useCallback(
    async (event: React.DragEvent<HTMLButtonElement>) => {
      event.preventDefault();
      const file = event.dataTransfer.files?.[0];
      if (!file) {
        return;
      }
      await loadInitImageFile(file);
    },
    [loadInitImageFile],
  );

  const handleSubmit = useCallback(async () => {
    if (!prompt.trim()) {
      toast.error(t('pages.video.error.enterPrompt'));
      return;
    }

    if (!selectedModel?.local_path) {
      toast.error(t('pages.video.error.selectDownloadedModel'));
      return;
    }

    setIsSubmitting(true);
    setVideoPath(null);

    try {
      const width = Number.parseInt(widthStr, 10) || 512;
      const height = Number.parseInt(heightStr, 10) || 512;
      const { operation_id } = await generateVideoMutation.mutateAsync({
        body: {
          model_id: selectedModelId || undefined,
          model: selectedModel.local_path,
          prompt,
          negative_prompt: negativePrompt || undefined,
          width,
          height,
          video_frames: frames,
          fps,
          cfg_scale: cfgScale,
          guidance,
          steps,
          seed: seed < 0 ? Math.floor(Math.random() * 2147483647) : seed,
          sample_method: sampleMethod === 'auto' ? undefined : sampleMethod,
          scheduler: scheduler === 'auto' ? undefined : scheduler,
          strength: initImageDataUri ? strength : undefined,
          init_image: initImageDataUri ?? undefined,
        } as never,
      });
      setTaskId(operation_id);
      setGenerationPhase('polling');
      pollAttempts.current = 0;
      toast.info(t('pages.video.toast.started', { frames, fps }));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(message);
    } finally {
      setIsSubmitting(false);
    }
  }, [
    cfgScale,
    fps,
    frames,
    guidance,
    heightStr,
    initImageDataUri,
    negativePrompt,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    selectedModelId,
    selectedModel,
    steps,
    strength,
    t,
    widthStr,
    generateVideoMutation,
  ]);

  useEffect(() => {
    if (!isPolling || !taskId || taskStatusUpdatedAt === 0) {
      return;
    }

    pollAttempts.current += 1;
    if (pollAttempts.current > MAX_POLL_ATTEMPTS) {
      toast.error(t('pages.video.toast.timedOut'));
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
      toast.error(taskStatus.error_msg ?? t('pages.video.error.generationFailed'));
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
    toast.error(t('pages.video.toast.pollingError', { message }));
    clearGenerationTask();
  }, [clearGenerationTask, isPolling, taskId, taskStatusError, t]);

  useEffect(() => {
    if (!isFetchingResult || !taskId) {
      return;
    }

    let cancelled = false;

    const loadResult = async () => {
      try {
        const detail = await getVideoGeneration(taskId);
        if (cancelled) {
          return;
        }

        const videoUrl = resolveMediaUrl(detail.video_url);
        mergeHistoryTask(detail);
        setSelectedHistoryTask(detail);
        setHistoryDialogOpen(true);

        if (videoUrl) {
          setVideoPath(videoUrl);
          toast.success(t('pages.video.toast.generated'));
        } else {
          toast.error(t('pages.video.toast.completedWithoutPath'));
        }
      } catch (error) {
        if (cancelled) {
          return;
        }

        const message = error instanceof Error ? error.message : String(error);
        toast.error(t('pages.video.toast.resultFetchFailed', { message }));
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
  }, [clearGenerationTask, isFetchingResult, mergeHistoryTask, refreshHistory, t, taskId]);

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

  const handleDownload = useCallback(() => {
    if (!videoPath) {
      return;
    }
    const anchor = document.createElement('a');
    anchor.href = videoPath;
    anchor.download = 'generated-video.mp4';
    anchor.click();
  }, [videoPath]);

  const widthValue = Number.parseInt(widthStr, 10) || 512;
  const heightValue = Number.parseInt(heightStr, 10) || 512;
  const clipDurationSeconds = frames / Math.max(fps, 1);

  const stageTitle = videoPath
    ? t('pages.video.stage.title.ready')
    : isGenerating
      ? t('pages.video.stage.title.rendering')
      : t('pages.video.stage.title.idle');

  const stageDescription = videoPath
    ? t('pages.video.stage.description.ready')
    : isGenerating
      ? t('pages.video.stage.description.rendering', { frames, fps })
      : t('pages.video.stage.description.idle');

  const stageStatus = videoPath
    ? t('pages.video.stage.status.ready')
    : isGenerating
      ? t('pages.video.stage.status.rendering')
      : prompt.trim()
        ? t('pages.video.stage.status.queued')
        : t('pages.video.stage.status.awaitingPrompt');

  const footerHint = selectedModel?.local_path
    ? videoPath
      ? t('pages.video.stage.footerHint.ready')
      : isGenerating
        ? t('pages.video.stage.footerHint.polling', { seconds: POLL_INTERVAL_MS / 1000 })
        : t('pages.video.stage.footerHint.estimate', {
            seconds: clipDurationSeconds.toFixed(1),
          })
    : t('pages.video.stage.footerHint.downloadFirst');

  return {
    advancedOpen,
    cfgScale,
    footerHint,
    fps,
    frames,
    guidance,
    handleCancel,
    handleDownload,
    handleInitImageChange,
    handleInitImageDrop,
    handleSubmit,
    heightStr,
    heightValue,
    hasSelectedModel: Boolean(selectedModel?.local_path),
    history,
    historyDialogOpen,
    historyError,
    historyLoading,
    immersivePreview,
    initImageDataUri,
    initImageInputRef,
    isGenerating,
    negativePrompt,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    selectedHistoryTask,
    setAdvancedOpen,
    setCfgScale,
    setFps,
    setFrames,
    setGuidance,
    setHeightStr,
    setHistoryDialogOpen,
    setImmersivePreview,
    setInitImageDataUri,
    setNegativePrompt,
    setPrompt,
    setSampleMethod,
    setScheduler,
    setSelectedHistoryTask,
    setSeed,
    setSteps,
    setStrength,
    setWidthStr,
    stageDescription,
    stageStatus,
    stageTitle,
    steps,
    strength,
    videoPath,
    widthStr,
    widthValue,
    openHistoryDetail,
  };
}
