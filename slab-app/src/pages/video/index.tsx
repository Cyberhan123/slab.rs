import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import { toast } from 'sonner';
import { ChevronDown, ChevronUp, Download, Film, Loader2, Upload, X } from 'lucide-react';
import api from '@/lib/api';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

// ── Constants ─────────────────────────────────────────────────────────────────

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL as string | undefined) ?? 'http://localhost:3000';

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
const MAX_POLL_ATTEMPTS = 300; // 10 minutes (video takes longer)

const DIFFUSION_BACKEND_ID = 'ggml.diffusion';

// ── Types ─────────────────────────────────────────────────────────────────────

type ModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  local_path: string | null;
};

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

export default function VideoPage() {
  // ── Model selection ─────────────────────────────────────────────────────────
  const [modelOptions, setModelOptions] = useState<ModelOption[]>([]);
  usePageHeader(PAGE_HEADER_META.video);
  const [selectedModelId, setSelectedModelId] = useState('');

  // ── Basic params ────────────────────────────────────────────────────────────
  const [prompt, setPrompt] = useState('');
  const [negativePrompt, setNegativePrompt] = useState('');
  const [widthStr, setWidthStr] = useState('512');
  const [heightStr, setHeightStr] = useState('512');
  const [frames, setFrames] = useState(16);
  const [fps, setFps] = useState(8);

  // ── Advanced params ─────────────────────────────────────────────────────────
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [cfgScale, setCfgScale] = useState(7.0);
  const [guidance, setGuidance] = useState(3.5);
  const [steps, setSteps] = useState(20);
  const [seed, setSeed] = useState(-1);
  const [sampleMethod, setSampleMethod] = useState('auto');
  const [scheduler, setScheduler] = useState('auto');
  const [strength, setStrength] = useState(0.75);

  // ── Init image ───────────────────────────────────────────────────────────────
  const [initImageDataUri, setInitImageDataUri] = useState<string | null>(null);
  const initImageInputRef = useRef<HTMLInputElement>(null);

  // ── Task state ──────────────────────────────────────────────────────────────
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [videoPath, setVideoPath] = useState<string | null>(null);
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

    const selectedModel = modelOptions.find((m) => m.id === selectedModelId);
    if (!selectedModel?.local_path) {
      toast.error('Selected model is not downloaded. Please download it first in Settings.');
      return;
    }

    setIsSubmitting(true);
    abortRef.current = false;
    setVideoPath(null);

    try {
      const [w, h] = [parseInt(widthStr, 10) || 512, parseInt(heightStr, 10) || 512];
      const response = await fetch(`${API_BASE_URL}/v1/video/generations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: selectedModel.local_path,
          prompt,
          negative_prompt: negativePrompt || undefined,
          width: w,
          height: h,
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
      toast.info(`Video generation started (${frames} frames at ${fps} fps)…`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(msg);
    } finally {
      setIsSubmitting(false);
    }
  }, [
    prompt, negativePrompt, widthStr, heightStr, frames, fps,
    cfgScale, guidance, steps, seed, sampleMethod, scheduler, strength,
    initImageDataUri, modelOptions, selectedModelId,
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
        toast.error('Video generation timed out');
        setIsPolling(false);
        setTaskId(null);
        return;
      }

      try {
        const statusRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}`);
        if (!statusRes.ok) throw new Error(`status ${statusRes.status}`);
        const status = (await statusRes.json()) as { status: string };

        if (status.status === 'failed') {
          toast.error('Video generation failed');
          setIsPolling(false);
          setTaskId(null);
          return;
        }

        if (status.status === 'succeeded') {
          const resultRes = await fetch(`${API_BASE_URL}/v1/tasks/${taskId}/result`);
          if (!resultRes.ok) throw new Error(`result ${resultRes.status}`);
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
  }, [isPolling, taskId]);

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

  // ── Render ───────────────────────────────────────────────────────────────────
  const isGenerating = isSubmitting || isPolling;

  return (
    <div className="h-full overflow-y-auto lg:overflow-hidden">
      <div className="container mx-auto flex h-full max-w-4xl flex-col px-4 py-6">
        <div className="grid grid-cols-1 gap-6 lg:min-h-0 lg:flex-1 lg:grid-cols-2">
        {/* ── Left panel: controls ── */}
          <div className="space-y-4 lg:min-h-0 lg:overflow-y-auto lg:pr-3">
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
            <Label htmlFor="v-prompt">Prompt</Label>
            <Textarea
              id="v-prompt"
              placeholder="a bird flying through clouds…"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              rows={3}
              className="resize-none"
            />
          </div>

          {/* Negative prompt */}
          <div className="space-y-1.5">
            <Label htmlFor="v-neg-prompt">Negative Prompt</Label>
            <Textarea
              id="v-neg-prompt"
              placeholder="blurry, low quality…"
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
                min={64} max={2048} step={64}
                value={widthStr}
                onChange={(e) => setWidthStr(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Height</Label>
              <Input
                type="number"
                min={64} max={2048} step={64}
                value={heightStr}
                onChange={(e) => setHeightStr(e.target.value)}
              />
            </div>
          </div>

          {/* Video params */}
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1.5">
              <div className="flex justify-between">
                <Label>Frames</Label>
                <span className="text-sm text-muted-foreground">{frames}</span>
              </div>
              <Slider min={4} max={120} step={4} value={[frames]}
                onValueChange={([v]) => setFrames(v)} />
            </div>
            <div className="space-y-1.5">
              <div className="flex justify-between">
                <Label>FPS</Label>
                <span className="text-sm text-muted-foreground">{fps}</span>
              </div>
              <Slider min={1} max={60} step={1} value={[fps]}
                onValueChange={([v]) => setFps(v)} />
            </div>
          </div>

          {/* Init image for video2video */}
          <div className="space-y-1.5">
            <Label>Init Image (optional, for video-to-video)</Label>
            {initImageDataUri ? (
              <div className="relative">
                <img
                  src={initImageDataUri}
                  alt="init"
                  className="w-full rounded-md border object-cover max-h-32"
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
                className="border-2 border-dashed rounded-md p-4 flex flex-col items-center gap-2 cursor-pointer hover:border-primary transition-colors"
                onClick={() => initImageInputRef.current?.click()}
              >
                <Upload className="h-5 w-5 text-muted-foreground" />
                <span className="text-xs text-muted-foreground">Click to upload</span>
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

          {/* Advanced params */}
          <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
            <CollapsibleTrigger asChild>
              <Button variant="ghost" className="w-full flex items-center justify-between px-2">
                <span className="text-sm font-medium">Advanced Parameters</span>
                {advancedOpen ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
              </Button>
            </CollapsibleTrigger>
            <CollapsibleContent className="space-y-4 pt-2">
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>CFG Scale</Label>
                  <span className="text-sm text-muted-foreground">{cfgScale.toFixed(1)}</span>
                </div>
                <Slider min={1} max={30} step={0.5} value={[cfgScale]}
                  onValueChange={([v]) => setCfgScale(v)} />
              </div>
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>Guidance (Flux/SD3)</Label>
                  <span className="text-sm text-muted-foreground">{guidance.toFixed(1)}</span>
                </div>
                <Slider min={0} max={10} step={0.1} value={[guidance]}
                  onValueChange={([v]) => setGuidance(v)} />
              </div>
              <div className="space-y-1.5">
                <div className="flex justify-between">
                  <Label>Steps</Label>
                  <span className="text-sm text-muted-foreground">{steps}</span>
                </div>
                <Slider min={1} max={50} step={1} value={[steps]}
                  onValueChange={([v]) => setSteps(v)} />
              </div>
              {initImageDataUri && (
                <div className="space-y-1.5">
                  <div className="flex justify-between">
                    <Label>Strength</Label>
                    <span className="text-sm text-muted-foreground">{strength.toFixed(2)}</span>
                  </div>
                  <Slider min={0} max={1} step={0.01} value={[strength]}
                    onValueChange={([v]) => setStrength(v)} />
                </div>
              )}
              <div className="space-y-1.5">
                <Label>Seed (-1 = random)</Label>
                <Input type="number" value={seed}
                  onChange={(e) => setSeed(parseInt(e.target.value, 10))} />
              </div>
              <div className="space-y-1.5">
                <Label>Sampler</Label>
                <Select value={sampleMethod} onValueChange={setSampleMethod}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {SAMPLE_METHODS.map((m) => (
                      <SelectItem key={m.value} value={m.value}>{m.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label>Scheduler</Label>
                <Select value={scheduler} onValueChange={setScheduler}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {SCHEDULERS.map((s) => (
                      <SelectItem key={s.value} value={s.value}>{s.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
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
                <><Loader2 className="mr-2 h-4 w-4 animate-spin" />Generating…</>
              ) : (
                <><Film className="mr-2 h-4 w-4" />Generate Video</>
              )}
            </Button>
            {isGenerating && (
              <Button variant="outline" onClick={handleCancel}>Cancel</Button>
            )}
          </div>
        </div>

        {/* ── Right panel: preview ── */}
          <div className="space-y-4 lg:min-h-0 lg:overflow-y-auto">
          {videoPath ? (
            <div className="space-y-3">
              <h3 className="font-medium">Generated Video</h3>
              <video
                src={`file://${videoPath}`}
                controls
                className="w-full rounded-lg border"
                autoPlay
                loop
              />
              <p className="text-xs text-muted-foreground break-all">{videoPath}</p>
              <Button
                variant="outline"
                className="w-full"
                onClick={() => {
                  const a = document.createElement('a');
                  a.href = `file://${videoPath}`;
                  a.download = 'generated-video.mp4';
                  a.click();
                }}
              >
                <Download className="mr-2 h-4 w-4" />
                Download
              </Button>
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center rounded-lg border border-dashed min-h-[400px] gap-4 text-muted-foreground">
              <Film className="h-12 w-12 opacity-30" />
              {isGenerating ? (
                <div className="text-center space-y-2">
                  <p className="text-sm font-medium">Generating {frames} frames…</p>
                  <p className="text-xs">This may take several minutes</p>
                  <div className="flex items-center justify-center gap-1 mt-2">
                    {[0, 1, 2].map((i) => (
                      <div
                        key={i}
                        className="h-2 w-2 bg-primary rounded-full animate-bounce"
                        style={{ animationDelay: `${i * 0.15}s` }}
                      />
                    ))}
                  </div>
                </div>
              ) : (
                <p className="text-sm">Generated video will appear here</p>
              )}
            </div>
          )}
          </div>
        </div>
      </div>
    </div>
  );
}
