import { type ReactNode, useCallback, useEffect, useRef, useState } from 'react';
import { useLocation } from 'react-router-dom';
import { toast } from 'sonner';
import {
  ChevronDown,
  ChevronUp,
  Download,
  History,
  ImageIcon,
  Lightbulb,
  Loader2,
  Sparkles,
  X,
  ZoomIn,
} from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
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
import { SplitWorkbench } from '@/components/ui/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { cn } from '@/lib/utils';
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
const DIMENSION_PRESETS = [
  { label: '1:1', width: 512, height: 512 },
  { label: '4:3', width: 768, height: 576 },
  { label: '16:9', width: 1024, height: 576 },
] as const;
const SIDEBAR_LABEL_CLASSNAME = 'text-[12px] font-semibold leading-4 text-[#191c1e]';
const SIDEBAR_INPUT_CLASSNAME =
  'h-10 w-full rounded-xl border-[#dbe4ea] bg-white px-3 text-sm text-[#191c1e] shadow-none focus-visible:border-[#64c3ba] focus-visible:ring-[3px] focus-visible:ring-[#0d9488]/12';
const SIDEBAR_TEXTAREA_CLASSNAME =
  'w-full rounded-xl border-[#dbe4ea] bg-white px-4 py-3 text-sm leading-5 text-[#191c1e] shadow-none resize-none focus-visible:border-[#64c3ba] focus-visible:ring-[3px] focus-visible:ring-[#0d9488]/12';

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
  const parsedWidth = Number.parseInt(widthStr, 10) || 512;
  const parsedHeight = Number.parseInt(heightStr, 10) || 512;
  const activeDimensionPreset =
    DIMENSION_PRESETS.find(
      (preset) => preset.width === parsedWidth && preset.height === parsedHeight,
    )?.label ?? null;

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

  return (
    <div className="h-full w-full overflow-y-auto bg-white">
      <div className="mx-auto flex min-h-full max-w-[1248px] flex-col px-4 py-4 sm:px-6">
        <SplitWorkbench
          className="h-full gap-6 xl:grid-cols-[320px_minmax(0,1fr)] xl:gap-0"
          sidebarClassName="space-y-0"
          mainClassName="min-h-full"
        sidebar={
          <aside className="flex h-full flex-col rounded-[28px] border border-[#eef2f7] bg-[#f2f4f6] px-5 py-5 xl:min-h-[780px] xl:rounded-none xl:border-0 xl:border-r xl:border-[#e2e8f0]/70 xl:px-6 xl:py-6">
            <div className="flex h-full flex-col">
              <div className="space-y-6">
              <div className="space-y-4">
                <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-[#64748b]">
                  Generation Parameters
                </p>
                <Tabs
                  value={mode}
                  onValueChange={(value) => setMode(value as typeof mode)}
                  className="gap-4"
                >
                  <TabsList className="grid h-auto w-full grid-cols-2 rounded-[16px] bg-transparent p-1">
                    <TabsTrigger
                      value="txt2img"
                      className="h-11 rounded-[16px] border border-transparent text-[14px] font-medium text-[#475569] shadow-none data-[state=active]:border-[#dbe4ea] data-[state=active]:bg-white data-[state=active]:text-[#0f172a] data-[state=active]:shadow-[0_1px_2px_rgba(0,0,0,0.05)]"
                    >
                      Text to Image
                    </TabsTrigger>
                    <TabsTrigger
                      value="img2img"
                      className="h-11 rounded-[16px] border border-transparent text-[14px] font-medium text-[#475569] shadow-none data-[state=active]:border-[#dbe4ea] data-[state=active]:bg-white data-[state=active]:text-[#0f172a] data-[state=active]:shadow-[0_1px_2px_rgba(0,0,0,0.05)]"
                    >
                      Image to Image
                    </TabsTrigger>
                  </TabsList>
                  <TabsContent value="txt2img" className="m-0" />
                  <TabsContent value="img2img" className="m-0">
                    <div className="space-y-2.5">
                      <Label className={SIDEBAR_LABEL_CLASSNAME}>
                        {initImageDataUri ? 'Init Image' : 'Upload Init Image'}
                      </Label>
                      <button
                        type="button"
                        className="group flex w-full flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-[#cbd5e1] bg-white px-4 py-4 text-center transition hover:border-[#5bc0b5] hover:bg-[#f8fcfb]"
                        onClick={() => initImageInputRef.current?.click()}
                      >
                        {initImageDataUri ? (
                          <div className="relative w-full overflow-hidden rounded-lg">
                            <img
                              src={initImageDataUri}
                              alt="init"
                              className="max-h-52 w-full rounded-lg object-cover"
                            />
                            <Button
                              type="button"
                              variant="pill"
                              size="icon-sm"
                              className="absolute top-2 right-2 border-white/80 bg-white/90 shadow-sm"
                              onClick={(event) => {
                                event.stopPropagation();
                                setInitImageDataUri(null);
                              }}
                            >
                              <X className="h-3.5 w-3.5" />
                            </Button>
                          </div>
                        ) : (
                          <>
                            <div className="flex size-12 items-center justify-center rounded-[14px] bg-[#f1f5f9] text-[#64748b] transition group-hover:bg-[#ebf7f5] group-hover:text-[#0d9488]">
                              <ImageIcon className="size-5" />
                            </div>
                            <div className="space-y-1">
                              <p className="text-sm font-medium text-[#191c1e]">
                                Click to choose an image
                              </p>
                              <p className="text-xs text-[#64748b]">
                                PNG/JPEG for img2img mode
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
                  </TabsContent>
                </Tabs>
              </div>

              <div className="space-y-2.5">
                <Label className={SIDEBAR_LABEL_CLASSNAME}>Model</Label>
                <Select
                  value={selectedModelId}
                  onValueChange={setSelectedModelId}
                  disabled={isBusy || modelOptions.length === 0}
                >
                  <SelectTrigger className={SIDEBAR_INPUT_CLASSNAME}>
                    <SelectValue
                      placeholder={catalogLoading ? 'Loading models...' : 'Select model'}
                    />
                  </SelectTrigger>
                  <SelectContent className="rounded-[16px] border-[#dbe4ea] bg-white shadow-[0_24px_48px_-34px_rgba(15,23,42,0.32)]">
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

              <div className="space-y-2.5">
                <Label htmlFor="prompt" className={SIDEBAR_LABEL_CLASSNAME}>
                  Prompt
                </Label>
                <Textarea
                  id="prompt"
                  placeholder="A cinematic portrait with moody rim light..."
                  rows={5}
                  value={prompt}
                  className={SIDEBAR_TEXTAREA_CLASSNAME}
                  onChange={(event) => setPrompt(event.target.value)}
                />
              </div>

              <div className="space-y-2.5">
                <Label htmlFor="negative-prompt" className={SIDEBAR_LABEL_CLASSNAME}>
                  Negative Prompt
                </Label>
                <Textarea
                  id="negative-prompt"
                  placeholder="blurry, low quality, distorted..."
                  rows={3}
                  value={negativePrompt}
                  className={SIDEBAR_TEXTAREA_CLASSNAME}
                  onChange={(event) => setNegativePrompt(event.target.value)}
                />
              </div>

              <div className="space-y-3">
                <div className="flex items-center justify-between gap-3">
                  <Label className={SIDEBAR_LABEL_CLASSNAME}>Dimensions</Label>
                  <span className="rounded-full bg-[#dff5f1] px-2 py-1 font-mono text-[10px] leading-none text-[#0d9488]">
                    {parsedWidth} x {parsedHeight}
                  </span>
                </div>
                <div className="grid grid-cols-3 gap-2">
                  {DIMENSION_PRESETS.map((preset) => {
                    const isActive = activeDimensionPreset === preset.label;

                    return (
                      <button
                        key={preset.label}
                        type="button"
                        aria-pressed={isActive}
                        className={cn(
                          'flex h-10 items-center justify-center rounded-xl border bg-white px-3 text-[11px] font-medium text-[#191c1e] transition',
                          isActive
                            ? 'border-[#74cec4] shadow-[0_1px_2px_rgba(0,0,0,0.05)]'
                            : 'border-[#e2e8f0] hover:border-[#cbd5e1]',
                        )}
                        onClick={() => handleDimensionPreset(preset.width, preset.height)}
                      >
                        {preset.label}
                      </button>
                    );
                  })}
                </div>
              </div>

              <div className="space-y-2.5">
                <Label className={SIDEBAR_LABEL_CLASSNAME}>Number of Images</Label>
                <Select
                  value={String(numImages)}
                  onValueChange={(value) => setNumImages(Number(value))}
                >
                  <SelectTrigger className={SIDEBAR_INPUT_CLASSNAME}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent className="rounded-[16px] border-[#dbe4ea] bg-white shadow-[0_24px_48px_-34px_rgba(15,23,42,0.32)]">
                    {[1, 2, 4].map((count) => (
                      <SelectItem key={count} value={String(count)}>
                        {count} {count === 1 ? 'Image' : 'Images'}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                <div className="border-t border-[#dbe4ea]">
                  <CollapsibleTrigger asChild>
                    <button
                      type="button"
                      className="flex w-full items-center justify-between py-3 text-left"
                    >
                      <span className="text-xs font-bold text-[#64748b]">
                        Advanced Settings
                      </span>
                      {advancedOpen ? (
                        <ChevronUp className="h-4 w-4 text-[#64748b]" />
                      ) : (
                        <ChevronDown className="h-4 w-4 text-[#64748b]" />
                      )}
                    </button>
                  </CollapsibleTrigger>
                </div>
                <CollapsibleContent className="space-y-4 pb-2">
                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-2.5">
                      <Label className={SIDEBAR_LABEL_CLASSNAME}>Width</Label>
                      <Input
                        type="number"
                        min={64}
                        max={2048}
                        step={64}
                        value={widthStr}
                        className={SIDEBAR_INPUT_CLASSNAME}
                        onChange={(event) => setWidthStr(event.target.value)}
                      />
                    </div>
                    <div className="space-y-2.5">
                      <Label className={SIDEBAR_LABEL_CLASSNAME}>Height</Label>
                      <Input
                        type="number"
                        min={64}
                        max={2048}
                        step={64}
                        value={heightStr}
                        className={SIDEBAR_INPUT_CLASSNAME}
                        onChange={(event) => setHeightStr(event.target.value)}
                      />
                    </div>
                  </div>
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

                  <div className="space-y-2.5">
                    <Label className={SIDEBAR_LABEL_CLASSNAME}>Seed (-1 random)</Label>
                    <Input
                      type="number"
                      value={seed}
                      className={SIDEBAR_INPUT_CLASSNAME}
                      onChange={(event) =>
                        setSeed(Number.parseInt(event.target.value, 10))
                      }
                    />
                  </div>

                  <div className="space-y-2.5">
                    <Label className={SIDEBAR_LABEL_CLASSNAME}>Sampler</Label>
                    <Select value={sampleMethod} onValueChange={setSampleMethod}>
                      <SelectTrigger className={SIDEBAR_INPUT_CLASSNAME}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent className="rounded-[16px] border-[#dbe4ea] bg-white shadow-[0_24px_48px_-34px_rgba(15,23,42,0.32)]">
                        {SAMPLE_METHODS.map((method) => (
                          <SelectItem key={method.value} value={method.value}>
                            {method.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="space-y-2.5">
                    <Label className={SIDEBAR_LABEL_CLASSNAME}>Scheduler</Label>
                    <Select value={scheduler} onValueChange={setScheduler}>
                      <SelectTrigger className={SIDEBAR_INPUT_CLASSNAME}>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent className="rounded-[16px] border-[#dbe4ea] bg-white shadow-[0_24px_48px_-34px_rgba(15,23,42,0.32)]">
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

              <div className="mt-auto pt-8">
                <Button
                  className="h-14 w-full rounded-xl bg-[linear-gradient(135deg,#00685f_0%,#008378_100%)] text-base font-semibold text-white shadow-[0_10px_15px_-3px_rgba(13,148,136,0.2),0_4px_6px_-4px_rgba(13,148,136,0.2)] hover:brightness-[1.03]"
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
                      Generate Images
                    </>
                  )}
                </Button>
                {isGenerating ? (
                  <Button
                    variant="ghost"
                    className="mt-3 h-10 w-full rounded-xl text-[#64748b] hover:bg-white/60 hover:text-[#0f172a]"
                    onClick={handleCancel}
                  >
                    Cancel generation
                  </Button>
                ) : null}
              </div>
            </div>
          </aside>
        }
        main={
          <section className="rounded-[28px] border border-[#eef2f7] bg-white xl:min-h-[780px] xl:rounded-none xl:border-0">
            {images.length === 0 ? (
              <div className="flex h-full min-h-[520px] items-center justify-center px-6 py-12 xl:min-h-[780px]">
                <div className="flex max-w-[448px] flex-col items-center gap-6 text-center">
                  <div className="relative flex items-center justify-center">
                    <div className="flex size-32 items-center justify-center rounded-full bg-[#f1f5f9] text-[#b8c4d2]">
                      <ImageIcon className="size-14 stroke-[1.5]" />
                    </div>
                    <div className="absolute -right-2 -bottom-2 flex size-12 items-center justify-center rounded-[14px] bg-[linear-gradient(135deg,#00685f_0%,#008378_100%)] text-white shadow-[0_10px_15px_-3px_rgba(13,148,136,0.24),0_4px_6px_-4px_rgba(13,148,136,0.22)]">
                      {isGenerating ? (
                        <Loader2 className="size-5 animate-spin" />
                      ) : (
                        <Sparkles className="size-5" />
                      )}
                    </div>
                  </div>
                  <div className="space-y-2">
                    <h2 className="text-[30px] font-semibold tracking-[-0.03em] text-[#0f172a]">
                      {isGenerating ? 'Generating images...' : 'Ready to create?'}
                    </h2>
                    <p className="mx-auto max-w-[320px] text-sm leading-6 text-[#64748b]">
                      {isGenerating
                        ? 'Your task is running. Generated images will appear here automatically.'
                        : 'Enter a prompt and adjust the parameters to see your imagination come to life.'}
                    </p>
                  </div>
                  {isGenerating ? (
                    <div className="rounded-full bg-[#f2f4f6] px-3 py-1 text-sm text-[#64748b]">
                      Task running
                    </div>
                  ) : (
                    <div className="flex flex-wrap items-center justify-center gap-4 text-sm text-[#94a3b8]">
                      <button
                        type="button"
                        className="inline-flex items-center gap-1.5 transition hover:text-[#0f172a]"
                      >
                        <History className="size-3.5" />
                        View History
                      </button>
                      <span className="size-1 rounded-full bg-[#e2e8f0]" />
                      <button
                        type="button"
                        className="inline-flex items-center gap-1.5 transition hover:text-[#0f172a]"
                      >
                        <Lightbulb className="size-3.5" />
                        Get Inspired
                      </button>
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <div className="flex h-full min-h-[520px] flex-col">
                <div className="border-b border-[#eef2f7] px-6 py-6 xl:px-10">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <h2 className="text-[24px] font-semibold tracking-[-0.02em] text-[#0f172a]">
                        Generated Images
                      </h2>
                      <p className="mt-1 text-sm text-[#64748b]">
                        Review the latest renders, zoom in for detail, or download the best take.
                      </p>
                    </div>
                    <span className="rounded-full bg-[#e7f7f4] px-3 py-1 text-sm font-medium text-[#0d9488]">
                      {images.length} {images.length === 1 ? 'image' : 'images'}
                    </span>
                  </div>
                </div>
                <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6 xl:px-10 xl:py-8">
                  <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
                    {images.map((image, index) => (
                      <figure
                        key={`${image.src}-${index}`}
                        className="group overflow-hidden rounded-[24px] border border-[#e2e8f0] bg-[#f8fafc] shadow-[0_18px_32px_-28px_rgba(15,23,42,0.28)]"
                      >
                        <div className="relative overflow-hidden bg-white">
                          <img
                            src={image.src}
                            alt={image.prompt}
                            className="max-h-[460px] w-full object-cover transition duration-300 group-hover:scale-[1.015]"
                            loading="lazy"
                          />
                          <div className="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/35 via-transparent to-transparent opacity-0 transition-opacity group-hover:opacity-100" />
                          <div className="absolute top-4 right-4 flex gap-2 opacity-100 transition md:opacity-0 md:group-hover:opacity-100">
                            <Button
                              variant="pill"
                              size="icon-sm"
                              className="border-white/80 bg-white/95 shadow-sm backdrop-blur"
                              onClick={() => setZoomedImage(image.src)}
                            >
                              <ZoomIn className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="pill"
                              size="icon-sm"
                              className="border-white/80 bg-white/95 shadow-sm backdrop-blur"
                              onClick={() => handleDownload(image.src, index)}
                            >
                              <Download className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                        <figcaption className="space-y-3 border-t border-[#e2e8f0]/80 bg-white px-4 py-4">
                          <p className="line-clamp-2 text-sm leading-6 text-[#334155]">
                            {image.prompt}
                          </p>
                          <div className="flex flex-wrap items-center gap-2 text-xs text-[#64748b]">
                            <Badge variant="chip">{image.mode}</Badge>
                            <span className="rounded-full bg-[#f2f4f6] px-2.5 py-1">
                              {image.width} x {image.height}
                            </span>
                          </div>
                        </figcaption>
                      </figure>
                    ))}
                  </div>
                </div>
              </div>
            )}
          </section>
        }
        />
      </div>

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
        <Label className={SIDEBAR_LABEL_CLASSNAME}>{label}</Label>
        <span className="text-[11px] font-medium text-[#64748b]">{value}</span>
      </div>
      {slider}
    </div>
  );
}
