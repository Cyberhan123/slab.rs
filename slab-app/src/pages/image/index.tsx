import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { toast } from 'sonner';
import {
  ChevronDown,
  ChevronUp,
  Download,
  ImageIcon,
  Loader2,
  Sparkles,
  Upload,
  X,
  ZoomIn,
} from 'lucide-react';
import api from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

// ── Constants ─────────────────────────────────────────────────────────────────

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

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

const DIFFUSION_BACKEND_ID = 'ggml.diffusion';

// ── Types ─────────────────────────────────────────────────────────────────────

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
  local_path: string | null;
};

interface GeneratedImage {
  src: string;
  prompt: string;
  width: number;
  height: number;
  mode: string;
}

interface TaskResult {
  image?: string;
  images?: string[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async function fileToDataUri(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function ImagePage() {
  // ── Mode ────────────────────────────────────────────────────────────────────
  const [mode, setMode] = useState<'txt2img' | 'img2img'>('txt2img');
  usePageHeader({
    ...PAGE_HEADER_META.image,
    subtitle:
      mode === 'img2img'
        ? 'Refine an input image with diffusion controls'
        : 'Generate images from text prompts',
  });

  // ── Model selection ─────────────────────────────────────────────────────────
  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  const [selectedModelId, setSelectedModelId] = useState('');
  // ── Basic params ────────────────────────────────────────────────────────────
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [widthStr, setWidthStr] = useState('512');
  const [heightStr, setHeightStr] = useState('512');
  const [numImages, setNumImages] = useState(1);

  // ── Advanced params ─────────────────────────────────────────────────────────
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [cfgScale, setCfgScale] = useState(7.0);
  const [guidance, setGuidance] = useState(3.5);
  const [steps, setSteps] = useState(20);
  const [seed, setSeed] = useState(-1);
  const [sampleMethod, setSampleMethod] = useState('auto');
  const [scheduler, setScheduler] = useState('auto');
  const [clipSkip, setClipSkip] = useState(0);
  const [eta, setEta] = useState(0.0);
  const [strength, setStrength] = useState(0.75);

  // ── Init image (img2img) ────────────────────────────────────────────────────
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const initImageInputRef = useRef<HTMLInputElement>(null);

  // ── Task state ──────────────────────────────────────────────────────────────
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [images, setImages] = useState<GeneratedImage[]>([]);
  const [zoomedImage, setZoomedImage] = useState<string | null>(null);
  const pollAttempts = useRef(0);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const abortRef = useRef(false);

  // ── Model loading ────────────────────────────────────────────────────────────
  const { data: catalogModels, isLoading: catalogLoading } = api.useQuery('get', '/v1/models');

  useEffect(() => {
    const models = Array.isArray(catalogModels) ? catalogModels : [];
    const diffusionModels = models
      .filter(
        (m) =>
          Array.isArray(m.backend_ids) &&
          m.backend_ids.includes(DIFFUSION_BACKEND_ID),
      )
      .map<ModelOption>((m) => ({
        id: m.id,
        label: m.display_name,
        downloaded: Boolean(m.local_path),
        pending: m.status === 'pending',
        local_path: m.local_path ?? null,
      }));
    setModelOptions(diffusionModels);
    if (diffusionModels.length > 0 && !selectedModelId) {
      const downloaded = diffusionModels.find((m) => m.downloaded);
      setSelectedModelId(downloaded?.id ?? diffusionModels[0].id);
    }
  }, [catalogModels, selectedModelId]);

  // ── Init image handling ──────────────────────────────────────────────────────
  const handleInitImageChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      try {
        const dataUri = await fileToDataUri(file);
        setInitImageDataUri(dataUri);
      } catch {
        toast.error('Failed to read image file');
      }
    },
    [],
  );

  // ── Submit ───────────────────────────────────────────────────────────────────
  const handleSubmit = useCallback(async () => {
    if (!prompt.trim()) {
      toast.error('Please enter a prompt');
      return;
    }
    if (mode === 'img2img' && !initImageDataUri) {
      toast.error('Please upload an init image for img2img mode');
      return;
    }

    const selectedModel = modelOptions.find((m) => m.id === selectedModelId);
    if (!selectedModel?.local_path) {
      toast.error('Selected model is not downloaded. Please download it first in Settings.');
      return;
    }

    setIsSubmitting(true);
    abortRef.current = false;

    try {
      const [w, h] = [parseInt(widthStr, 10) || 512, parseInt(heightStr, 10) || 512];
      const response = await fetch(`${API_BASE_URL}/v1/images/generations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: selectedModel.local_path,
          prompt,
          negative_prompt: negativePrompt || undefined,
          n: numImages,
          width: w,
          height: h,
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
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(msg);
    } finally {
      setIsSubmitting(false);
    }
  }, [
    prompt, negativePrompt, numImages, widthStr, heightStr,
    cfgScale, guidance, steps, seed, sampleMethod, scheduler,
    clipSkip, eta, strength, mode, initImageDataUri,
    modelOptions, selectedModelId,
  ]);

  // ── Polling ──────────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!isPolling || !taskId) return;

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
        if (!statusRes.ok) throw new Error(`status ${statusRes.status}`);
        const status = (await statusRes.json()) as { status: string };

        if (status.status === 'failed') {
          toast.error('Image generation failed');
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        if (status.status === 'succeeded') {
          const resultRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/result`);
          if (!resultRes.ok) throw new Error(`result ${resultRes.status}`);
          const result = (await resultRes.json()) as TaskResult;

          const srcs: string[] = result.images ?? (result.image ? [result.image] : []);
          const [w, h] = [parseInt(widthStr, 10) || 512, parseInt(heightStr, 10) || 512];
          const newImages: GeneratedImage[] = srcs.map((src) => ({
            src,
            prompt,
            width: w,
            height: h,
            mode,
          }));
          setImages((prev) => [...newImages, ...prev]);
          toast.success(`Generated ${newImages.length} image${newImages.length !== 1 ? 's' : ''}!`);
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        // Still running — poll again.
        pollTimer.current = setTimeout(poll, POLL_INTERVAL_MS);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        toast.error(`Polling error: ${msg}`);
        setIsPolling(false);
        setTaskId(null);
      }
    };

    pollTimer.current = setTimeout(poll, POLL_INTERVAL_MS);
    return () => {
      if (pollTimer.current) clearTimeout(pollTimer.current);
    };
  }, [isPolling, taskId, prompt, widthStr, heightStr, mode]);

  const handleCancel = useCallback(async () => {
    abortRef.current = true;
    if (pollTimer.current) clearTimeout(pollTimer.current);
    if (taskId) {
      try {
        await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/cancel`, { method: 'POST' });
      } catch (err) {
        // Best-effort — don't block UI cleanup on server errors.
        console.error('Failed to cancel task', err);
      }
    }
    setIsPolling(false);
    setTaskId(null);
  }, [taskId]);

  const handleDownload = useCallback((src: string, index: number) => {
    const a = document.createElement('a');
    a.href = src;
    a.download = `generated-${index + 1}.png`;
    a.click();
  }, []);

  // ── Render ───────────────────────────────────────────────────────────────────
  const isGenerating = isSubmitting || isPolling;

  return (
    <div className="h-full overflow-y-auto lg:overflow-hidden">
      <div className="container mx-auto flex h-full max-w-6xl flex-col px-4 py-6">
        <div className="grid grid-cols-1 gap-6 lg:min-h-0 lg:flex-1 lg:grid-cols-3">
          {/* ── Left panel: controls ── */}
          <div className="space-y-4 lg:col-span-1 lg:min-h-0 lg:overflow-y-auto lg:pr-3">
          {/* Mode tabs */}
          <Tabs value={mode} onValueChange={(v) => setMode(v as typeof mode)}>
            <TabsList className="w-full">
              <TabsTrigger value="txt2img" className="flex-1">Text → Image</TabsTrigger>
              <TabsTrigger value="img2img" className="flex-1">Image → Image</TabsTrigger>
            </TabsList>

            <TabsContent value="txt2img" className="mt-0" />
            <TabsContent value="img2img" className="mt-0">
              {/* Init image upload */}
              <div className="mt-3 space-y-2">
                <Label>Init Image</Label>
                {initImageDataUri ? (
                  <div className="relative">
                    <img
                      src={initImageDataUri}
                      alt="init"
                      className="w-full rounded-md border object-cover max-h-40"
                    />
                    <Button
                      size="icon"
                      variant="destructive"
                      className="absolute top-1 right-1 h-6 w-6"
                      onClick={() => setInitImageDataUri(null)}
                    >
                      <X className="h-3 w-3" />
                    </Button>
                  </div>
                ) : (
                  <div
                    className="border-2 border-dashed rounded-md p-6 flex flex-col items-center gap-2 cursor-pointer hover:border-primary transition-colors"
                    onClick={() => initImageInputRef.current?.click()}
                  >
                    <Upload className="h-6 w-6 text-muted-foreground" />
                    <span className="text-sm text-muted-foreground">
                      Click to upload PNG / JPEG
                    </span>
                  </div>
                )}
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

          {/* Model selector */}
          <div className="space-y-1.5">
            <Label>Model</Label>
            <Select value={selectedModelId} onValueChange={setSelectedModelId}>
              <SelectTrigger>
                <SelectValue placeholder={catalogLoading ? 'Loading…' : 'Select a model'} />
              </SelectTrigger>
              <SelectContent>
                {modelOptions.length === 0 && (
                  <SelectItem value="__none" disabled>
                    No diffusion models found
                  </SelectItem>
                )}
                {modelOptions.map((m) => (
                  <SelectItem key={m.id} value={m.id} disabled={!m.downloaded}>
                    <span className="flex items-center gap-2">
                      {m.label}
                      {!m.downloaded && (
                        <Badge variant="outline" className="text-xs">Not downloaded</Badge>
                      )}
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Prompt */}
          <div className="space-y-1.5">
            <Label htmlFor="prompt">Prompt</Label>
            <Textarea
              id="prompt"
              placeholder="a cat sitting on a rooftop at sunset…"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={3}
              className="resize-none"
            />
          </div>

          {/* Negative prompt */}
          <div className="space-y-1.5">
            <Label htmlFor="neg-prompt">Negative Prompt</Label>
            <Textarea
              id="neg-prompt"
              placeholder="blurry, low quality, ugly…"
              value={negativePrompt}
              onChange={(e) => setNegativePrompt(e.target.value)}
              rows={2}
              className="resize-none"
            />
          </div>

          {/* Size */}
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <Label>Width</Label>
              <Input
                type="number"
                min={64}
                max={2048}
                step={64}
                value={widthStr}
                onChange={(e) => setWidthStr(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Height</Label>
              <Input
                type="number"
                min={64}
                max={2048}
                step={64}
                value={heightStr}
                onChange={(e) => setHeightStr(e.target.value)}
              />
            </div>
          </div>

          {/* Count */}
          <div className="space-y-1.5">
            <Label>Number of Images</Label>
            <Select value={String(numImages)} onValueChange={(v) => setNumImages(Number(v))}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {[1, 2, 4].map((n) => (
                  <SelectItem key={n} value={String(n)}>{n}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Advanced params */}
          <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
            <CollapsibleTrigger asChild>
              <Button variant="ghost" className="w-full flex items-center justify-between px-2">
                <span className="text-sm font-medium">Advanced Parameters</span>
                {advancedOpen ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent className="space-y-4 pt-2">
              {/* CFG Scale */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>CFG Scale</Label>
                  <span className="text-sm text-muted-foreground">{cfgScale.toFixed(1)}</span>
                </div>
                <Slider
                  min={1}
                  max={30}
                  step={0.5}
                  value={[cfgScale]}
                  onValueChange={([v]) => setCfgScale(v)}
                />
              </div>

              {/* Guidance (distilled, for Flux/SD3) */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>Guidance (Flux/SD3)</Label>
                  <span className="text-sm text-muted-foreground">{guidance.toFixed(1)}</span>
                </div>
                <Slider
                  min={0}
                  max={10}
                  step={0.1}
                  value={[guidance]}
                  onValueChange={([v]) => setGuidance(v)}
                />
              </div>

              {/* Steps */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>Steps</Label>
                  <span className="text-sm text-muted-foreground">{steps}</span>
                </div>
                <Slider
                  min={1}
                  max={50}
                  step={1}
                  value={[steps]}
                  onValueChange={([v]) => setSteps(v)}
                />
              </div>

              {/* Strength (img2img only) */}
              {mode === 'img2img' && (
                <div className="space-y-1.5">
                  <div className="flex justify-between">
                    <Label>Strength</Label>
                    <span className="text-sm text-muted-foreground">{strength.toFixed(2)}</span>
                  </div>
                  <Slider
                    min={0}
                    max={1}
                    step={0.01}
                    value={[strength]}
                    onValueChange={([v]) => setStrength(v)}
                  />
                </div>
              )}

              {/* Seed */}
              <div className="space-y-1.5">
                <Label>Seed (-1 = random)</Label>
                <Input
                  type="number"
                  value={seed}
                  onChange={(e) => setSeed(parseInt(e.target.value, 10))}
                />
              </div>

              {/* Sampler */}
              <div className="space-y-1.5">
                <Label>Sampler</Label>
                <Select value={sampleMethod} onValueChange={setSampleMethod}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {SAMPLE_METHODS.map((m) => (
                      <SelectItem key={m.value} value={m.value}>{m.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Scheduler */}
              <div className="space-y-1.5">
                <Label>Scheduler</Label>
                <Select value={scheduler} onValueChange={setScheduler}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {SCHEDULERS.map((s) => (
                      <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* CLIP skip */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>CLIP Skip</Label>
                  <span className="text-sm text-muted-foreground">{clipSkip}</span>
                </div>
                <Slider
                  min={0}
                  max={12}
                  step={1}
                  value={[clipSkip]}
                  onValueChange={([v]) => setClipSkip(v)}
                />
              </div>

              {/* Eta */}
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>Eta (DDIM)</Label>
                  <span className="text-sm text-muted-foreground">{eta.toFixed(2)}</span>
                </div>
                <Slider
                  min={0}
                  max={1}
                  step={0.01}
                  value={[eta]}
                  onValueChange={([v]) => setEta(v)}
                />
              </div>
            </CollapsibleContent>
          </Collapsible>

          <Separator />

          {/* Generate button */}
          <div className="flex gap-2">
            <Button
              className="flex-1"
              onClick={handleSubmit}
              disabled={isGenerating || !prompt.trim()}
            >
              {isGenerating ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Generating…
                </>
              ) : (
                <>
                  <Sparkles className="mr-2 h-4 w-4" />
                  Generate
                </>
              )}
            </Button>
            {isGenerating && (
              <Button variant="outline" onClick={handleCancel}>
                Cancel
              </Button>
            )}
          </div>
        </div>

          {/* ── Right panel: gallery ── */}
          <div className="min-h-0 lg:col-span-2 lg:overflow-y-auto">
          {images.length === 0 ? (
            <div className="flex flex-col items-center justify-center rounded-lg border border-dashed h-full min-h-[400px] gap-4 text-muted-foreground">
              <ImageIcon className="h-12 w-12 opacity-30" />
              <p className="text-sm">Generated images will appear here</p>
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-4">
              {images.map((img, i) => (
                <div key={i} className="group relative rounded-lg overflow-hidden border bg-muted">
                  <img
                    src={img.src}
                    alt={img.prompt}
                    className="w-full object-cover"
                    loading="lazy"
                  />
                  <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                    <Button
                      size="icon"
                      variant="secondary"
                      onClick={() => setZoomedImage(img.src)}
                    >
                      <ZoomIn className="h-4 w-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="secondary"
                      onClick={() => handleDownload(img.src, i)}
                    >
                      <Download className="h-4 w-4" />
                    </Button>
                  </div>
                  <div className="p-2 text-xs text-muted-foreground truncate">
                    {img.prompt}
                  </div>
                </div>
              ))}
            </div>
          )}
          </div>
        </div>

        {/* Zoom dialog */}
        <Dialog open={!!zoomedImage} onOpenChange={(open) => { if (!open) setZoomedImage(null); }}>
          <DialogContent className="max-w-3xl p-2">
            {zoomedImage && (
              <img src={zoomedImage} alt="preview" className="w-full rounded" />
            )}
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
}
