import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toast } from 'sonner';
import {
  ChevronDown,
  ChevronUp,
  Download,
  Film,
  Loader2,
  X,
} from 'lucide-react';

import api from '@/lib/api';
import { toCatalogModelList } from '@/lib/api/models';
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
                        src={initImageDataUri}
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
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label>{label}</Label>
        <span className="text-xs font-medium text-muted-foreground">{value}</span>
      </div>
      {slider}
    </div>
  );
}
