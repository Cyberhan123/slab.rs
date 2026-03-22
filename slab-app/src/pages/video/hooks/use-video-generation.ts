import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';

import api from '@/lib/api';
import { toCatalogModelList } from '@/lib/api/models';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import {
  API_BASE_URL,
  DIFFUSION_BACKEND_ID,
  MAX_POLL_ATTEMPTS,
  POLL_INTERVAL_MS,
  type ModelOption,
} from '../const';

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
  const [selectedModelId, setSelectedModelId] = useState('');
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
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [videoPath, setVideoPath] = useState<string | null>(null);
  const [immersivePreview, setImmersivePreview] = useState(false);

  const initImageInputRef = useRef<HTMLInputElement>(null);
  const pollAttempts = useRef(0);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef(false);

  usePageHeader(PAGE_HEADER_META.video);

  const { data: catalogModels, isLoading: catalogLoading } = api.useQuery(
    'get',
    '/v1/models',
  );

  useEffect(() => {
    const diffusionModels = toCatalogModelList(catalogModels)
      .filter((model) => model.backend_id === DIFFUSION_BACKEND_ID)
      .map<ModelOption>((model) => ({
        id: model.id,
        label: model.display_name,
        downloaded: Boolean(model.local_path),
        local_path: model.local_path ?? null,
      }));

    setModelOptions(diffusionModels);
    if (diffusionModels.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = diffusionModels.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      const downloaded = diffusionModels.find((model) => model.downloaded);
      setSelectedModelId(downloaded?.id ?? diffusionModels[0].id);
    }
  }, [catalogModels, selectedModelId]);

  const selectedModel = useMemo(
    () => modelOptions.find((model) => model.id === selectedModelId),
    [modelOptions, selectedModelId],
  );

  const isGenerating = isSubmitting || isPolling;

  const summaryItems = useMemo(
    () => [
      { label: 'Frames', value: frames },
      { label: 'FPS', value: fps },
      { label: 'Size', value: `${widthStr || '--'} x ${heightStr || '--'}` },
      { label: 'Init Image', value: initImageDataUri ? 'Attached' : 'None' },
    ],
    [fps, frames, heightStr, initImageDataUri, widthStr],
  );

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
    abortRef.current = false;
    setVideoPath(null);

    try {
      const width = Number.parseInt(widthStr, 10) || 512;
      const height = Number.parseInt(heightStr, 10) || 512;
      const response = await fetch(`${API_BASE_URL}/v1/video/generations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
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
        toast.error('Video generation timed out');
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
          toast.error('Video generation failed');
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        if (status.status === 'succeeded') {
          const resultRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/result`);
          if (!resultRes.ok) {
            throw new Error(`result ${resultRes.status}`);
          }
          const result = (await resultRes.json()) as { video_path?: string };
          if (result.video_path) {
            setVideoPath(result.video_path);
            toast.success('Video generated!');
          }
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
  }, [isPolling, taskId]);

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
    catalogLoading,
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
    immersivePreview,
    initImageDataUri,
    initImageInputRef,
    isGenerating,
    modelOptions,
    negativePrompt,
    prompt,
    sampleMethod,
    scheduler,
    seed,
    selectedModelId,
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
    setSelectedModelId,
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
