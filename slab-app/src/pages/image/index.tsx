import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { toast } from 'sonner';
import {
  ChevronDown,
  ChevronUp,
  Download,
  ImageIcon,
  Loader2,
  Sparkles,
  X,
  ZoomIn,
} from 'lucide-react';

import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent } from '@/components/ui/dialog';
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
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
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { useImageModelPreparation } from './hooks/use-image-model-preparation';

const API_BASE_URL =
  (import.meta.env.VITE_API_BASE_URL as string | undefined) ??
  'http://localhost:3000';

const SAMPLE_METHODS = [
  { value: 'auto', label: 'Auto' },
  { value: 'euler', label: 'Euler' },
  { value: 'euler_a', label: 'Euler A' },
  { value: 'heun', label: 'Heun' },
  { value: 'dpm2', label: 'DPM2' },
  { value: 'dpm++2s_a', label: 'DPM++ 2S a' },
  { value: 'dpm++2m', label: 'DPM++ 2M' },
  { value: 'dpm++2mv2', label: 'DPM++ 2M v2' },
  { value: 'lcm', label: 'LCM' },
  { value: 'ipndm', label: 'iPNDM' },
  { value: 'ipndm_v', label: 'iPNDM V' },
] as const;

const SCHEDULERS = [
  { value: 'auto', label: 'Auto' },
  { value: 'discrete', label: 'Discrete' },
  { value: 'karras', label: 'Karras' },
  { value: 'exponential', label: 'Exponential' },
  { value: 'ays', label: 'AYS' },
  { value: 'gits', label: 'GITS' },
] as const;

const POLL_INTERVAL_MS = 2_000;
const MAX_POLL_ATTEMPTS = 150;

type GeneratedImage = {
  src: string;
  prompt: string;
  width: number;
  height: number;
  mode: 'txt2img' | 'img2img';
};

type TaskResult = {
  image?: string;
  images?: string[];
};

type ImageRouteState = {
  prompt?: string;
};

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export default function ImagePage() {
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

  const summaryItems = useMemo(
    () => [
      { label: 'Mode', value: mode },
      { label: 'Size', value: `${widthStr || '--'} x ${heightStr || '--'}` },
      { label: 'Batch', value: numImages },
      { label: 'Result Count', value: images.length },
    ],
    [heightStr, images.length, mode, numImages, widthStr],
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

  return (
    <div className="h-full w-full overflow-y-auto">
      <SplitWorkbench
        className="h-full"
        sidebar={
          <>
            <SoftPanel className="space-y-4">
              <Tabs value={mode} onValueChange={(value) => setMode(value as typeof mode)}>
                <TabsList className="grid w-full grid-cols-2">
                  <TabsTrigger value="txt2img">Text to Image</TabsTrigger>
                  <TabsTrigger value="img2img">Image to Image</TabsTrigger>
                </TabsList>
                <TabsContent value="txt2img" className="mt-0" />
                <TabsContent value="img2img" className="mt-4">
                  <UploadDropzone
                    title={initImageDataUri ? 'Init image ready' : 'Upload init image'}
                    description={
                      initImageDataUri
                        ? 'Click to replace image'
                        : 'PNG/JPEG for img2img mode'
                    }
                    actionLabel="Choose image"
                    preview={
                      initImageDataUri ? (
                        <div className="relative">
                          <img
                            src={initImageDataUri}
                            alt="init"
                            className="max-h-52 w-full rounded-[20px] object-cover"
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
                </TabsContent>
              </Tabs>

              <div className="space-y-2">
                <Label>Model</Label>
                <Select
                  value={selectedModelId}
                  onValueChange={setSelectedModelId}
                  disabled={isBusy || modelOptions.length === 0}
                >
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
                        <SelectItem key={model.id} value={model.id}>
                          <span className="flex min-w-0 items-center gap-2">
                            <span className="truncate">{model.label}</span>
                            {model.pending ? <Badge variant="chip">Downloading</Badge> : null}
                            {!model.downloaded ? <Badge variant="chip">Not local</Badge> : null}
                          </span>
                        </SelectItem>
                      ))
                    )}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <Label htmlFor="prompt">Prompt</Label>
                <Textarea
                  id="prompt"
                  variant="soft"
                  placeholder="A cinematic portrait with moody rim light..."
                  rows={4}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="negative-prompt">Negative Prompt</Label>
                <Textarea
                  id="negative-prompt"
                  variant="soft"
                  placeholder="blurry, low quality, distorted..."
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

              <div className="space-y-2">
                <Label>Batch</Label>
                <Select
                  value={String(numImages)}
                  onValueChange={(value) => setNumImages(Number(value))}
                >
                  <SelectTrigger variant="soft" className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent variant="soft">
                    {[1, 2, 4].map((count) => (
                      <SelectItem key={count} value={String(count)}>
                        {count}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

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
                  {mode === 'img2img' ? (
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

                  <SliderField
                    label="CLIP Skip"
                    value={clipSkip}
                    slider={
                      <Slider
                        min={0}
                        max={12}
                        step={1}
                        value={[clipSkip]}
                        onValueChange={([value]) => setClipSkip(value)}
                      />
                    }
                  />
                  <SliderField
                    label="Eta (DDIM)"
                    value={eta.toFixed(2)}
                    slider={
                      <Slider
                        min={0}
                        max={1}
                        step={0.01}
                        value={[eta]}
                        onValueChange={([value]) => setEta(value)}
                      />
                    }
                  />
                </CollapsibleContent>
              </Collapsible>

              <div className="flex gap-2 pt-2">
                <Button
                  variant="cta"
                  size="pill"
                  className="flex-1"
                  onClick={handleSubmit}
                  disabled={isBusy || !prompt.trim() || !selectedModelId}
                >
                  {isPreparingModel ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Preparing model...
                    </>
                  ) : isGenerating ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Generating...
                    </>
                  ) : (
                    <>
                      <Sparkles className="h-4 w-4" />
                      Generate
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

            <CompactConfigSummary title="Generation Snapshot" items={summaryItems} />
          </>
        }
        main={
          <div className="flex h-full min-h-[540px] flex-col gap-4">
            {images.length === 0 ? (
              <StageEmptyState
                title={isGenerating ? 'Generating images...' : 'No images yet'}
                description={
                  isGenerating
                    ? 'Your task is running. Generated images will appear here automatically.'
                    : 'Tune your prompt and settings on the left, then start generation.'
                }
                icon={ImageIcon}
              />
            ) : (
              <Card variant="soft" className="flex min-h-0 flex-1 flex-col overflow-hidden">
                <CardHeader className="border-b border-border/60">
                  <CardTitle className="flex items-center gap-2 text-lg">
                    Results
                    <Badge variant="counter">{images.length}</Badge>
                  </CardTitle>
                </CardHeader>
                <CardContent className="min-h-0 flex-1 overflow-y-auto pt-5">
                  <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
                    {images.map((image, index) => (
                      <figure
                        key={`${image.src}-${index}`}
                        className="group workspace-soft-panel overflow-hidden rounded-[22px] p-0"
                      >
                        <div className="relative">
                          <img
                            src={image.src}
                            alt={image.prompt}
                            className="max-h-[460px] w-full object-cover"
                            loading="lazy"
                          />
                          <div className="pointer-events-none absolute inset-0 bg-black/35 opacity-0 transition-opacity group-hover:opacity-100" />
                          <div className="absolute top-3 right-3 flex gap-2 opacity-0 transition-opacity group-hover:opacity-100">
                            <Button
                              variant="pill"
                              size="icon-sm"
                              onClick={() => setZoomedImage(image.src)}
                            >
                              <ZoomIn className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="pill"
                              size="icon-sm"
                              onClick={() => handleDownload(image.src, index)}
                            >
                              <Download className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                        <figcaption className="space-y-1 px-3 py-3 text-xs text-muted-foreground">
                          <div className="line-clamp-2">{image.prompt}</div>
                          <div className="flex items-center gap-2">
                            <Badge variant="chip">{image.mode}</Badge>
                            <span>
                              {image.width} x {image.height}
                            </span>
                          </div>
                        </figcaption>
                      </figure>
                    ))}
                  </div>
                </CardContent>
              </Card>
            )}
          </div>
        }
      />

      <Dialog
        open={Boolean(zoomedImage)}
        onOpenChange={(open) => {
          if (!open) {
            setZoomedImage(null);
          }
        }}
      >
        <DialogContent className="max-w-4xl p-2">
          {zoomedImage ? (
            <img src={zoomedImage} alt="preview" className="w-full rounded-xl" />
          ) : null}
        </DialogContent>
      </Dialog>
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
