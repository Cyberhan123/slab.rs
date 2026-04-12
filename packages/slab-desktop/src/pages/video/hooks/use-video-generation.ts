import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';

import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import api from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';
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
type OperationAcceptedResponse = components['schemas']['OperationAcceptedResponse'];
type TaskResponse = components['schemas']['TaskResponse'];
type TaskResultPayload = components['schemas']['TaskResultPayload'];
type VideoGenerationRequest = components['schemas']['VideoGenerationRequest'];

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export function useVideoGeneration() {
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

  const initImageInputRef = useRef<HTMLInputElement>(null);
  const pollAttempts = useRef(0);

  usePageHeader(PAGE_HEADER_META.video);

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
  const generateVideoMutation = api.useMutation('post', '/v1/video/generations') as unknown as {
    mutateAsync: (options: { body: VideoGenerationRequest }) => Promise<OperationAcceptedResponse>;
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
  const isGenerating = isSubmitting || generationPhase !== 'idle';
  const headerModelPicker = useMemo(
    () => ({
      type: 'select' as const,
      value: selectedModelId,
      options: modelOptions.map((model) => ({
        id: model.id,
        label: model.downloaded ? model.label : `${model.label} (Download in Hub)`,
        disabled: !model.downloaded,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: 'Video Models',
      placeholder: 'Select model',
      loading: catalogLoading,
      disabled: catalogLoading || isGenerating || !modelOptions.some((model) => model.downloaded),
      emptyLabel: 'No diffusion models',
    }),
    [catalogLoading, isGenerating, modelOptions, selectedModelId, setSelectedModelId],
  );

  usePageHeaderControl(headerModelPicker);

  const clearGenerationTask = useCallback(() => {
    pollAttempts.current = 0;
    setGenerationPhase('idle');
    setTaskId(null);
  }, []);

  const loadInitImageFile = useCallback(async (file: File) => {
    if (!file.type.startsWith('image/')) {
      toast.error('Please choose an image file');
      return;
    }
    try {
      const dataUri = await fileToDataUri(file);
      setInitImageDataUri(dataUri);
    } catch {
      toast.error('Failed to read image file');
    }
  }, []);

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
      toast.error('Please enter a prompt');
      return;
    }

    if (!selectedModel?.local_path) {
      toast.error(
        'Selected model is not downloaded. Please download it first in Settings.',
      );
      return;
    }

    setIsSubmitting(true);
    setVideoPath(null);

    try {
      const width = Number.parseInt(widthStr, 10) || 512;
      const height = Number.parseInt(heightStr, 10) || 512;
      const { operation_id } = await generateVideoMutation.mutateAsync({
        body: {
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
        },
      });
      setTaskId(operation_id);
      setGenerationPhase('polling');
      pollAttempts.current = 0;
      toast.info(`Video generation started (${frames} frames at ${fps} fps)...`);
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
    selectedModel,
    steps,
    strength,
    widthStr,
    generateVideoMutation,
  ]);

  useEffect(() => {
    if (!isPolling || !taskId || taskStatusUpdatedAt === 0) {
      return;
    }

    pollAttempts.current += 1;
    if (pollAttempts.current > MAX_POLL_ATTEMPTS) {
      toast.error('Video generation timed out');
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
      toast.error(taskStatus.error_msg ?? 'Video generation failed');
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

    if (taskResult.video_path) {
      setVideoPath(taskResult.video_path);
      toast.success('Video generated!');
    } else {
      toast.error('Video generation completed without a video path');
    }

    clearGenerationTask();
  }, [clearGenerationTask, isFetchingResult, taskId, taskResult, taskResultUpdatedAt]);

  useEffect(() => {
    if (!isFetchingResult || !taskId || !taskResultError) {
      return;
    }

    const message = taskResultError instanceof Error ? taskResultError.message : String(taskResultError);
    toast.error(`Failed to fetch video result: ${message}`);
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

  const handleDownload = useCallback(() => {
    if (!videoPath) {
      return;
    }
    const anchor = document.createElement('a');
    anchor.href = `file://${videoPath}`;
    anchor.download = 'generated-video.mp4';
    anchor.click();
  }, [videoPath]);

  const widthValue = Number.parseInt(widthStr, 10) || 512;
  const heightValue = Number.parseInt(heightStr, 10) || 512;
  const clipDurationSeconds = frames / Math.max(fps, 1);

  const stageTitle = videoPath
    ? 'Render Ready'
    : isGenerating
      ? 'Rendering Preview'
      : 'Preview Canvas';

  const stageDescription = videoPath
    ? 'Your generated clip is ready to review, resize in stage, or download locally.'
    : isGenerating
      ? `Generating ${frames} frames at ${fps} fps. Slab is polling the runtime for completion.`
      : 'Generated video will appear here after processing. Ready for cinematic render.';

  const stageStatus = videoPath
    ? 'Render complete'
    : isGenerating
      ? 'Generating'
      : prompt.trim()
        ? 'Ready to render'
        : 'Awaiting prompt';

  const footerHint = selectedModel?.local_path
    ? videoPath
      ? 'Generated clip is saved locally and available for download.'
      : isGenerating
        ? `Polling every ${POLL_INTERVAL_MS / 1000} seconds until the runtime finishes.`
        : `Estimated clip length: ${clipDurationSeconds.toFixed(1)} seconds.`
    : 'Download a local diffusion model in Settings before starting a render.';

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
    immersivePreview,
    initImageDataUri,
    initImageInputRef,
    isGenerating,
    negativePrompt,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    setAdvancedOpen,
    setCfgScale,
    setFps,
    setFrames,
    setGuidance,
    setHeightStr,
    setImmersivePreview,
    setInitImageDataUri,
    setNegativePrompt,
    setPrompt,
    setSampleMethod,
    setScheduler,
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
  };
}
