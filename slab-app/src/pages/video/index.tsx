import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';
import {
  ChevronDown,
  ChevronUp,
  Download,
  Film,
  ImagePlus,
  Loader2,
  Maximize2,
  X,
} from 'lucide-react';

import api from '@/lib/api';
import { toCatalogModelList } from '@/lib/api/models';
import { cn } from '@/lib/utils';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Slider } from '@/components/ui/slider';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Textarea } from '@/components/ui/textarea';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible';
import {
  CompactConfigSummary,
  SoftPanel,
  SplitWorkbench,
  StageEmptyState,
  UploadDropzone,
} from '@/components/ui/workspace';

const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined) ??
  'http://localhost:3000';

const SAMPLE_METHODS = [
  { value: 'auto', label: 'Auto' },
  { value: 'euler', label: 'Euler' },
  { value: 'euler_a', label: 'Euler A' },
  { value: 'lcm', label: 'LCM' },
  { value: 'dpm++2m', label: 'DPM++ 2M' },
] as const;

const SCHEDULERS = [
  { value: 'auto', label: 'Auto' },
  { value: 'discrete', label: 'Discrete' },
  { value: 'karras', label: 'Karras' },
] as const;

const FRAME_OPTIONS = [8, 16, 24, 32, 48, 60, 80, 120] as const;
const FPS_OPTIONS = [6, 8, 12, 16, 24, 30, 48, 60] as const;

const POLL_INTERVAL_MS = 2_000;
const MAX_POLL_ATTEMPTS = 300;
const DIFFUSION_BACKEND_ID = 'ggml.diffusion';

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  local_path: string | null;
};

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export default function VideoPage() {
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
  return (
    <VideoWorkbench
      advancedOpen={advancedOpen}
      catalogLoading={catalogLoading}
      footerHint={footerHint}
      fps={fps}
      frames={frames}
      handleCancel={handleCancel}
      handleDownload={handleDownload}
      handleInitImageChange={handleInitImageChange}
      handleInitImageDrop={handleInitImageDrop}
      handleSubmit={handleSubmit}
      heightStr={heightStr}
      heightValue={heightValue}
      immersivePreview={immersivePreview}
      initImageDataUri={initImageDataUri}
      initImageInputRef={initImageInputRef}
      isGenerating={isGenerating}
      modelOptions={modelOptions}
      negativePrompt={negativePrompt}
      prompt={prompt}
      sampleMethod={sampleMethod}
      scheduler={scheduler}
      selectedModelId={selectedModelId}
      setAdvancedOpen={setAdvancedOpen}
      setCfgScale={setCfgScale}
      setFps={setFps}
      setFrames={setFrames}
      setGuidance={setGuidance}
      setHeightStr={setHeightStr}
      setImmersivePreview={setImmersivePreview}
      setInitImageDataUri={setInitImageDataUri}
      setNegativePrompt={setNegativePrompt}
      setPrompt={setPrompt}
      setSampleMethod={setSampleMethod}
      setScheduler={setScheduler}
      setSeed={setSeed}
      setSelectedModelId={setSelectedModelId}
      setSteps={setSteps}
      setStrength={setStrength}
      setWidthStr={setWidthStr}
      stageDescription={stageDescription}
      stageStatus={stageStatus}
      stageTitle={stageTitle}
      strength={strength}
      steps={steps}
      cfgScale={cfgScale}
      guidance={guidance}
      seed={seed}
      videoPath={videoPath}
      widthStr={widthStr}
      widthValue={widthValue}
    />
  );

  return (
    <div className="h-full w-full overflow-y-auto">
      <SplitWorkbench
        className="h-full"
        sidebar={
          <>
            <SoftPanel className="space-y-4">
              <div className="space-y-2">
                <Label>Model</Label>
                <Select value={selectedModelId} onValueChange={setSelectedModelId}>
                  <SelectTrigger variant="pill" size="pill" className="w-full">
                    <SelectValue
                      placeholder={catalogLoading ? 'Loading models...' : 'Select model'}
                    />
                  </SelectTrigger>
                  <SelectContent variant="pill">
                    {modelOptions.length === 0 ? (
                      <SelectItem value="__none" disabled>
                        No diffusion models found
                      </SelectItem>
                    ) : (
                      modelOptions.map((model) => (
                        <SelectItem
                          key={model.id}
                          value={model.id}
                          disabled={!model.downloaded}
                        >
                          <span className="flex min-w-0 items-center gap-2">
                            <span className="truncate">{model.label}</span>
                            {!model.downloaded ? (
                              <Badge variant="chip">Not downloaded</Badge>
                            ) : null}
                          </span>
                        </SelectItem>
                      ))
                    )}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label htmlFor="video-prompt">Prompt</Label>
                <Textarea
                  id="video-prompt"
                  variant="soft"
                  placeholder="A bird flying through clouds in golden-hour light..."
                  rows={4}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="video-negative">Negative Prompt</Label>
                <Textarea
                  id="video-negative"
                  variant="soft"
                  placeholder="blurry, low quality, artifacts..."
                  rows={3}
                  value={negativePrompt}
                  onChange={(event) => setNegativePrompt(event.target.value)}
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-2">
                  <Label>Width</Label>
                  <Input
                    variant="soft"
                    type="number"
                    min={64}
                    max={2048}
                    step={64}
                    value={widthStr}
                    onChange={(event) => setWidthStr(event.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label>Height</Label>
                  <Input
                    variant="soft"
                    type="number"
                    min={64}
                    max={2048}
                    step={64}
                    value={heightStr}
                    onChange={(event) => setHeightStr(event.target.value)}
                  />
                </div>
              </div>

              <SliderField
                label="Frames"
                value={frames}
                slider={
                  <Slider
                    min={4}
                    max={120}
                    step={4}
                    value={[frames]}
                    onValueChange={([value]) => setFrames(value)}
                  />
                }
              />

              <SliderField
                label="FPS"
                value={fps}
                slider={
                  <Slider
                    min={1}
                    max={60}
                    step={1}
                    value={[fps]}
                    onValueChange={([value]) => setFps(value)}
                  />
                }
              />

              <UploadDropzone
                title={initImageDataUri ? 'Init image ready' : 'Init image (optional)'}
                description="Preserves current backend semantics: this maps to `init_image`."
                actionLabel="Choose image"
                preview={
                  initImageDataUri ? (
                    <div className="relative">
                      <img
                        src={initImageDataUri ?? undefined}
                        alt="init"
                        className="max-h-48 w-full rounded-[20px] object-cover"
                      />
                      <Button
                        type="button"
                        variant="destructive"
                        size="icon-sm"
                        className="absolute top-2 right-2"
                        onClick={(event) => {
                          event.stopPropagation();
                          setInitImageDataUri(null);
                        }}
                      >
                        <X className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  ) : undefined
                }
                onClick={() => initImageInputRef.current?.click()}
              />
              <input
                ref={initImageInputRef}
                type="file"
                accept="image/png,image/jpeg"
                className="hidden"
                onChange={handleInitImageChange}
              />

              <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                <CollapsibleTrigger asChild>
                  <Button variant="quiet" className="w-full justify-between">
                    Advanced Parameters
                    {advancedOpen ? (
                      <ChevronUp className="h-4 w-4" />
                    ) : (
                      <ChevronDown className="h-4 w-4" />
                    )}
                  </Button>
                </CollapsibleTrigger>
                <CollapsibleContent className="space-y-4 pt-3">
                  <SliderField
                    label="CFG Scale"
                    value={cfgScale.toFixed(1)}
                    slider={
                      <Slider
                        min={1}
                        max={30}
                        step={0.5}
                        value={[cfgScale]}
                        onValueChange={([value]) => setCfgScale(value)}
                      />
                    }
                  />
                  <SliderField
                    label="Guidance"
                    value={guidance.toFixed(1)}
                    slider={
                      <Slider
                        min={0}
                        max={10}
                        step={0.1}
                        value={[guidance]}
                        onValueChange={([value]) => setGuidance(value)}
                      />
                    }
                  />
                  <SliderField
                    label="Steps"
                    value={steps}
                    slider={
                      <Slider
                        min={1}
                        max={50}
                        step={1}
                        value={[steps]}
                        onValueChange={([value]) => setSteps(value)}
                      />
                    }
                  />
                  {initImageDataUri ? (
                    <SliderField
                      label="Strength"
                      value={strength.toFixed(2)}
                      slider={
                        <Slider
                          min={0}
                          max={1}
                          step={0.01}
                          value={[strength]}
                          onValueChange={([value]) => setStrength(value)}
                        />
                      }
                    />
                  ) : null}

                  <div className="space-y-2">
                    <Label>Seed (-1 random)</Label>
                    <Input
                      variant="soft"
                      type="number"
                      value={seed}
                      onChange={(event) =>
                        setSeed(Number.parseInt(event.target.value, 10))
                      }
                    />
                  </div>

                  <div className="space-y-2">
                    <Label>Sampler</Label>
                    <Select value={sampleMethod} onValueChange={setSampleMethod}>
                      <SelectTrigger variant="soft" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent variant="soft">
                        {SAMPLE_METHODS.map((method) => (
                          <SelectItem key={method.value} value={method.value}>
                            {method.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-2">
                    <Label>Scheduler</Label>
                    <Select value={scheduler} onValueChange={setScheduler}>
                      <SelectTrigger variant="soft" className="w-full">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent variant="soft">
                        {SCHEDULERS.map((schedulerItem) => (
                          <SelectItem
                            key={schedulerItem.value}
                            value={schedulerItem.value}
                          >
                            {schedulerItem.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </CollapsibleContent>
              </Collapsible>

              <div className="flex gap-2 pt-2">
                <Button
                  variant="cta"
                  size="pill"
                  className="flex-1"
                  onClick={handleSubmit}
                  disabled={isGenerating || !prompt.trim()}
                >
                  {isGenerating ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Generating...
                    </>
                  ) : (
                    <>
                      <Film className="h-4 w-4" />
                      Generate Video
                    </>
                  )}
                </Button>
                {isGenerating ? (
                  <Button variant="pill" size="pill" onClick={handleCancel}>
                    Cancel
                  </Button>
                ) : null}
              </div>
            </SoftPanel>

            <CompactConfigSummary title="Video Snapshot" items={summaryItems} />
          </>
        }
        main={
          videoPath ? (
            <Card variant="soft" className="h-full min-h-[540px]">
              <CardHeader className="border-b border-border/60">
                <CardTitle className="flex items-center gap-2 text-lg">
                  Generated Video
                  <Badge variant="counter">Ready</Badge>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4 pt-5">
                <video
                  src={`file://${videoPath}`}
                  controls
                  autoPlay
                  loop
                  className="w-full rounded-[20px] border border-border/60"
                />
                <div className="rounded-2xl bg-[var(--surface-soft)] px-3 py-2 text-xs text-muted-foreground break-all">
                  {videoPath}
                </div>
                <Button
                  variant="pill"
                  size="pill"
                  onClick={() => {
                    const anchor = document.createElement('a');
                    anchor.href = `file://${videoPath}`;
                    anchor.download = 'generated-video.mp4';
                    anchor.click();
                  }}
                >
                  <Download className="h-4 w-4" />
                  Download
                </Button>
              </CardContent>
            </Card>
          ) : (
            <StageEmptyState
              title={isGenerating ? `Generating ${frames} frames...` : 'No video generated yet'}
              description={
                isGenerating
                  ? 'The current task is running and status is polled continuously.'
                  : 'Use the left workbench to compose a prompt and launch generation.'
              }
              icon={Film}
            />
          )
        }
      />
    </div>
  );
}

function SliderField({
  label,
  value,
  slider,
}: {
  label: string;
  value: string | number;
  slider: ReactNode;
}) {
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <FieldLabel>{label}</FieldLabel>
        <span className="font-mono text-[12px] font-semibold text-[#00685f]">
          {value}
        </span>
      </div>
      {slider}
    </div>
  );
}

function FieldLabel({
  className,
  ...props
}: React.ComponentProps<typeof Label>) {
  return (
    <Label
      className={cn(
        'text-[11px] font-bold uppercase tracking-[0.18em] text-slate-500',
        className,
      )}
      {...props}
    />
  );
}

function ResolutionSliderField({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: string;
  min: number;
  max: number;
  step: number;
  onChange: (value: string) => void;
}) {
  const numericValue = Number.parseInt(value, 10);
  const resolvedValue = Number.isFinite(numericValue) ? numericValue : min;

  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <FieldLabel>{label}</FieldLabel>
        <span className="font-mono text-[12px] font-semibold text-[#00685f]">
          {resolvedValue}
        </span>
      </div>
      <Slider
        min={min}
        max={max}
        step={step}
        value={[resolvedValue]}
        onValueChange={([nextValue]) => onChange(String(nextValue))}
        className="[&_[data-slot=slider-range]]:bg-[#0f766e] [&_[data-slot=slider-thumb]]:border-[#0f766e] [&_[data-slot=slider-track]]:bg-slate-200"
      />
    </div>
  );
}

function ToolbarIconButton({
  icon: Icon,
  label,
  active = false,
  disabled = false,
  onClick,
}: {
  icon: (props: { className?: string }) => ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'flex size-10 items-center justify-center rounded-2xl text-slate-600 transition',
        active && 'bg-white text-[#00685f] shadow-[0_12px_24px_-18px_rgba(15,23,42,0.45)]',
        !active && 'hover:bg-white/70 hover:text-slate-900',
        disabled && 'cursor-not-allowed opacity-35 hover:bg-transparent hover:text-slate-600',
      )}
    >
      <Icon className="h-[18px] w-[18px]" />
    </button>
  );
}

function StatusMetric({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div className="space-y-1.5">
      <p className="text-[10px] font-bold uppercase tracking-[0.2em] text-slate-500">
        {label}
      </p>
      <p className="text-sm font-semibold text-slate-900">{value}</p>
    </div>
  );
}

type VideoWorkbenchProps = {
  advancedOpen: boolean;
  catalogLoading: boolean;
  cfgScale: number;
  footerHint: string;
  fps: number;
  frames: number;
  guidance: number;
  handleCancel: () => void | Promise<void>;
  handleDownload: () => void;
  handleInitImageChange: (event: React.ChangeEvent<HTMLInputElement>) => void | Promise<void>;
  handleInitImageDrop: (event: React.DragEvent<HTMLButtonElement>) => void | Promise<void>;
  handleSubmit: () => void | Promise<void>;
  heightStr: string;
  heightValue: number;
  immersivePreview: boolean;
  initImageDataUri: string | null;
  initImageInputRef: React.RefObject<HTMLInputElement | null>;
  isGenerating: boolean;
  modelOptions: ModelOption[];
  negativePrompt: string;
  prompt: string;
  sampleMethod: string;
  scheduler: string;
  seed: number;
  selectedModelId: string;
  setAdvancedOpen: (open: boolean) => void;
  setCfgScale: (value: number) => void;
  setFps: (value: number) => void;
  setFrames: (value: number) => void;
  setGuidance: (value: number) => void;
  setHeightStr: (value: string) => void;
  setImmersivePreview: React.Dispatch<React.SetStateAction<boolean>>;
  setInitImageDataUri: (value: string | null) => void;
  setNegativePrompt: (value: string) => void;
  setPrompt: (value: string) => void;
  setSampleMethod: (value: string) => void;
  setScheduler: (value: string) => void;
  setSeed: (value: number) => void;
  setSelectedModelId: (value: string) => void;
  setSteps: (value: number) => void;
  setStrength: (value: number) => void;
  setWidthStr: (value: string) => void;
  stageDescription: string;
  stageStatus: string;
  stageTitle: string;
  steps: number;
  strength: number;
  videoPath: string | null;
  widthStr: string;
  widthValue: number;
};

function VideoWorkbench({
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
}: VideoWorkbenchProps) {
  return (
    <div className="h-full w-full overflow-y-auto bg-white xl:overflow-hidden">
      <div className="mx-auto flex min-h-full w-full max-w-[1200px] flex-col px-4 py-4 sm:px-6 xl:h-full xl:min-h-0 xl:py-6">
        <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[378px_minmax(0,1fr)]">
          <aside className="flex h-full min-h-[520px] flex-col rounded-[28px] border border-slate-200/80 bg-[linear-gradient(180deg,rgba(242,244,246,0.96),rgba(248,250,252,0.96))] p-6 shadow-[0_20px_50px_-38px_rgba(15,23,42,0.35)] xl:min-h-0 xl:overflow-hidden">
            <div className="pb-6">
              <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-slate-500">
                Configuration
              </p>
            </div>

            <div className="min-h-0 flex-1 space-y-6 overflow-y-auto pr-2">
              <div className="space-y-2.5">
                <FieldLabel htmlFor="video-prompt">Creative Prompt</FieldLabel>
                <Textarea
                  id="video-prompt"
                  variant="soft"
                  placeholder="Describe the scene in cinematic detail..."
                  rows={4}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  className="min-h-[112px] rounded-[22px] border-slate-200/80 bg-white/78 px-4 py-4 text-[15px] leading-6 text-slate-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.9)] placeholder:text-slate-400"
                />
              </div>

              <div className="space-y-2.5">
                <FieldLabel htmlFor="video-negative">Negative Prompt</FieldLabel>
                <Input
                  id="video-negative"
                  variant="soft"
                  placeholder="Blurry, low quality, distorted..."
                  value={negativePrompt}
                  onChange={(event) => setNegativePrompt(event.target.value)}
                  className="h-14 rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-[15px] text-slate-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.9)] placeholder:text-slate-400"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <ResolutionSliderField
                  label="Width"
                  value={widthStr}
                  min={64}
                  max={2048}
                  step={64}
                  onChange={setWidthStr}
                />
                <ResolutionSliderField
                  label="Height"
                  value={heightStr}
                  min={64}
                  max={2048}
                  step={64}
                  onChange={setHeightStr}
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-2.5">
                  <FieldLabel>Frames</FieldLabel>
                  <Select value={String(frames)} onValueChange={(value) => setFrames(Number(value))}>
                    <SelectTrigger
                      variant="soft"
                      className="h-14 w-full rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-base font-semibold text-slate-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent variant="soft">
                      {FRAME_OPTIONS.map((option) => (
                        <SelectItem key={option} value={String(option)}>
                          {option}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>

                <div className="space-y-2.5">
                  <FieldLabel>FPS</FieldLabel>
                  <Select value={String(fps)} onValueChange={(value) => setFps(Number(value))}>
                    <SelectTrigger
                      variant="soft"
                      className="h-14 w-full rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-base font-semibold text-slate-900 shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent variant="soft">
                      {FPS_OPTIONS.map((option) => (
                        <SelectItem key={option} value={String(option)}>
                          {option}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div className="space-y-2.5">
                <FieldLabel>Reference Image</FieldLabel>
                <button
                  type="button"
                  onClick={() => initImageInputRef.current?.click()}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={handleInitImageDrop}
                  className="group flex w-full flex-col items-center justify-center gap-4 rounded-[22px] border-2 border-dashed border-slate-300/80 bg-white/52 px-5 py-7 text-center transition hover:border-[#0d9488]/45 hover:bg-white/72"
                >
                  {initImageDataUri ? (
                    <div className="relative w-full overflow-hidden rounded-[18px] border border-white/70 bg-white/80 shadow-[0_18px_30px_-24px_rgba(15,23,42,0.35)]">
                      <img
                        src={initImageDataUri}
                        alt="Reference"
                        className="h-36 w-full object-cover"
                      />
                      <div className="flex items-center justify-between gap-3 px-4 py-3 text-left">
                        <div>
                          <p className="text-sm font-semibold text-slate-900">Reference frame ready</p>
                          <p className="text-xs text-slate-500">
                            Slab will use this image as the starting frame.
                          </p>
                        </div>
                        <Button
                          type="button"
                          variant="destructive"
                          size="icon-sm"
                          className="shrink-0"
                          onClick={(event) => {
                            event.stopPropagation();
                            setInitImageDataUri(null);
                          }}
                        >
                          <X className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <div className="flex size-14 items-center justify-center rounded-full bg-white text-slate-500 shadow-[0_18px_30px_-24px_rgba(15,23,42,0.35)]">
                        <ImagePlus className="h-6 w-6" />
                      </div>
                      <div className="space-y-1">
                        <p className="text-sm font-medium text-slate-700">
                          Drop PNG/JPG or click to upload
                        </p>
                        <p className="text-xs text-slate-500">
                          Optional starting frame for motion generation.
                        </p>
                      </div>
                    </>
                  )}
                </button>
                <input
                  ref={initImageInputRef}
                  type="file"
                  accept="image/png,image/jpeg"
                  className="hidden"
                  onChange={handleInitImageChange}
                />
              </div>

              <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                <CollapsibleTrigger asChild>
                  <button
                    type="button"
                    className="flex w-full items-center justify-between rounded-[18px] border border-slate-200/80 bg-white/72 px-4 py-3 text-sm font-semibold text-slate-700 shadow-[inset_0_1px_0_rgba(255,255,255,0.85)] transition hover:border-slate-300 hover:text-slate-900"
                  >
                    Advanced Parameters
                    {advancedOpen ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
                  </button>
                </CollapsibleTrigger>

                <CollapsibleContent className="space-y-5 pt-4">
                  <div className="space-y-2.5">
                    <FieldLabel>Render Model</FieldLabel>
                    <Select value={selectedModelId} onValueChange={setSelectedModelId}>
                      <SelectTrigger
                        variant="soft"
                        className="h-12 w-full rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                      >
                        <SelectValue
                          placeholder={catalogLoading ? 'Loading models...' : 'Select model'}
                        />
                      </SelectTrigger>
                      <SelectContent variant="soft">
                        {modelOptions.length === 0 ? (
                          <SelectItem value="__none" disabled>
                            No diffusion models found
                          </SelectItem>
                        ) : (
                          modelOptions.map((model) => (
                            <SelectItem
                              key={model.id}
                              value={model.id}
                              disabled={!model.downloaded}
                            >
                              <span className="flex min-w-0 items-center gap-2">
                                <span className="truncate">{model.label}</span>
                                {!model.downloaded ? <Badge variant="chip">Not downloaded</Badge> : null}
                              </span>
                            </SelectItem>
                          ))
                        )}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid gap-4 sm:grid-cols-2">
                    <SliderField
                      label="CFG Scale"
                      value={cfgScale.toFixed(1)}
                      slider={
                        <Slider
                          min={1}
                          max={30}
                          step={0.5}
                          value={[cfgScale]}
                          onValueChange={([value]) => setCfgScale(value)}
                        />
                      }
                    />
                    <SliderField
                      label="Guidance"
                      value={guidance.toFixed(1)}
                      slider={
                        <Slider
                          min={0}
                          max={10}
                          step={0.1}
                          value={[guidance]}
                          onValueChange={([value]) => setGuidance(value)}
                        />
                      }
                    />
                    <SliderField
                      label="Steps"
                      value={steps}
                      slider={
                        <Slider
                          min={1}
                          max={50}
                          step={1}
                          value={[steps]}
                          onValueChange={([value]) => setSteps(value)}
                        />
                      }
                    />
                    {initImageDataUri ? (
                      <SliderField
                        label="Strength"
                        value={strength.toFixed(2)}
                        slider={
                          <Slider
                            min={0}
                            max={1}
                            step={0.01}
                            value={[strength]}
                            onValueChange={([value]) => setStrength(value)}
                          />
                        }
                      />
                    ) : null}
                  </div>

                  <div className="grid gap-4 sm:grid-cols-2">
                    <div className="space-y-2.5">
                      <FieldLabel>Seed (-1 random)</FieldLabel>
                      <Input
                        variant="soft"
                        type="number"
                        value={seed}
                        onChange={(event) => setSeed(Number.parseInt(event.target.value, 10))}
                        className="h-12 rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                      />
                    </div>

                    <div className="space-y-2.5">
                      <FieldLabel>Sampler</FieldLabel>
                      <Select value={sampleMethod} onValueChange={setSampleMethod}>
                        <SelectTrigger
                          variant="soft"
                          className="h-12 w-full rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent variant="soft">
                          {SAMPLE_METHODS.map((method) => (
                            <SelectItem key={method.value} value={method.value}>
                              {method.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="space-y-2.5 sm:col-span-2">
                      <FieldLabel>Scheduler</FieldLabel>
                      <Select value={scheduler} onValueChange={setScheduler}>
                        <SelectTrigger
                          variant="soft"
                          className="h-12 w-full rounded-[18px] border-slate-200/80 bg-white/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_rgba(255,255,255,0.9)]"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent variant="soft">
                          {SCHEDULERS.map((schedulerItem) => (
                            <SelectItem key={schedulerItem.value} value={schedulerItem.value}>
                              {schedulerItem.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                </CollapsibleContent>
              </Collapsible>
            </div>

            <div className="space-y-3 pt-1">
              <Button
                variant="cta"
                size="pill"
                className="h-[68px] w-full rounded-[18px] text-base font-semibold shadow-[0_24px_40px_-18px_color-mix(in_oklab,var(--brand-teal)_58%,transparent)]"
                onClick={handleSubmit}
                disabled={isGenerating || !prompt.trim()}
              >
                {isGenerating ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Generating...
                  </>
                ) : (
                  <>
                    <Film className="h-4 w-4" />
                    Generate Video
                  </>
                )}
              </Button>
              {isGenerating ? (
                <Button
                  variant="pill"
                  size="pill"
                  className="h-11 w-full rounded-[16px]"
                  onClick={handleCancel}
                >
                  Cancel current render
                </Button>
              ) : null}
            </div>
          </aside>

          <section className="flex min-h-[520px] flex-col gap-6 xl:min-h-0">
            <div
              className={cn(
                'relative flex min-h-[420px] flex-1 items-center justify-center overflow-hidden rounded-[32px] border border-slate-200/70 bg-[#eceef0] p-6 shadow-[0_32px_80px_-56px_rgba(15,23,42,0.45)] xl:min-h-0',
              )}
              style={{
                backgroundImage:
                  'radial-gradient(circle at center, rgba(0,104,95,0.12) 0%, rgba(255,255,255,0) 24%), linear-gradient(135deg, rgba(255,255,255,0.88) 0%, rgba(235,239,241,0.92) 40%, rgba(255,255,255,0.9) 100%)',
              }}
            >
              <div className="absolute inset-0 opacity-70 [background:radial-gradient(circle_at_top_right,rgba(15,23,42,0.06),transparent_38%),radial-gradient(circle_at_bottom_left,rgba(255,255,255,0.88),transparent_34%)]" />

              {videoPath ? (
                <div className="relative z-10 w-full max-w-[640px] space-y-4">
                  <div className="overflow-hidden rounded-[28px] border border-white/50 bg-black/88 shadow-[0_32px_80px_-42px_rgba(15,23,42,0.6)]">
                    <video
                      src={`file://${videoPath}`}
                      controls
                      autoPlay
                      loop
                      className={cn(
                        'w-full bg-black',
                        immersivePreview ? 'h-[520px] object-cover' : 'max-h-[520px] object-contain',
                      )}
                    />
                  </div>
                </div>
              ) : (
                <div className="relative z-10 flex max-w-[340px] flex-col items-center gap-6 text-center">
                  <div className="relative">
                    <div className="absolute inset-[-26px] rounded-full bg-[#00685f]/18 blur-3xl" />
                    <div className="relative flex size-24 items-center justify-center rounded-[32px] bg-white text-[#00685f] shadow-[0_28px_60px_-36px_rgba(15,23,42,0.45)]">
                      {isGenerating ? <Loader2 className="h-10 w-10 animate-spin" /> : <Film className="h-10 w-10" />}
                    </div>
                  </div>

                  <div className="space-y-3">
                    <h2 className="text-[32px] font-semibold tracking-[-0.035em] text-slate-900">
                      {stageTitle}
                    </h2>
                    <p className="text-sm leading-7 text-slate-600">{stageDescription}</p>
                  </div>
                </div>
              )}

              <div className="absolute bottom-8 left-1/2 z-20 -translate-x-1/2">
                <div className="flex items-center gap-2 rounded-[20px] border border-white/45 bg-white/72 px-4 py-3 backdrop-blur-xl shadow-[0_24px_50px_-34px_rgba(15,23,42,0.42)]">
                  <ToolbarIconButton
                    icon={Maximize2}
                    label="Toggle stage scale"
                    active={immersivePreview}
                    onClick={() => setImmersivePreview((current) => !current)}
                  />
                  <ToolbarIconButton
                    icon={Download}
                    label="Download video"
                    disabled={!videoPath}
                    onClick={handleDownload}
                  />
                </div>
              </div>
            </div>

            <div className="rounded-[22px] border border-slate-200/80 bg-[linear-gradient(180deg,rgba(242,244,246,0.95),rgba(248,250,252,0.92))] px-5 py-4 shadow-[0_18px_42px_-34px_rgba(15,23,42,0.28)]">
              <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                <div className="grid gap-4 sm:grid-cols-3">
                  <StatusMetric label="Render Status" value={stageStatus} />
                  <StatusMetric label="Clip Spec" value={`${frames} frames • ${fps} fps`} />
                  <StatusMetric label="Canvas" value={`${widthValue} x ${heightValue}`} />
                </div>
                <p className="text-xs font-medium text-slate-600 lg:text-right">{footerHint}</p>
              </div>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
