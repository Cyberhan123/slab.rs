import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { toast } from 'sonner';
import { Download, ImageIcon, Loader2, Sparkles, X, ZoomIn } from 'lucide-react';
import api from '@/lib/api';
import { Dialog, DialogContent } from '@/components/ui/dialog';

// ── Types ─────────────────────────────────────────────────────────────────────

interface GeneratedImage {
  src: string;
  prompt: string;
  size: string;
}

interface TaskStatusResponse {
  status: string;
}

interface TaskImageResult {
  image?: string;
  images?: string[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const SIZES = [
  { value: '256x256', label: '256 × 256' },
  { value: '512x512', label: '512 × 512' },
  { value: '1024x1024', label: '1024 × 1024' },
] as const;

const COUNTS = ['1', '2', '4'] as const;

const POLL_INTERVAL_MS = 2_000;
const MAX_POLL_ATTEMPTS = 150; // 5 minutes

// ── Component ─────────────────────────────────────────────────────────────────

export default function Image() {
  const [prompt, setPrompt] = useState('');
  const [numImages, setNumImages] = useState<string>('1');
  const [size, setSize] = useState<string>('512x512');
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isPolling, setIsPolling] = useState(false);
  const [images, setImages] = useState<GeneratedImage[]>([]);
  const [lightboxSrc, setLightboxSrc] = useState<string | null>(null);
  const abortRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const generateMutation = api.useMutation('post', '/v1/images/generations');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const getResultMutation = api.useMutation('get', '/v1/tasks/{id}/result');
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel');

  const isBusy = isSubmitting || isPolling;

  // Clear any pending poll timer when the component unmounts.
  useEffect(() => {
    return () => {
      abortRef.current = true;
      if (timerRef.current !== null) {
        clearTimeout(timerRef.current);
      }
    };
  }, []);

  const pollTaskStatus = useCallback(async (id: string, capturedPrompt: string, capturedSize: string) => {
    setIsPolling(true);
    abortRef.current = false;
    let attempts = 0;

    const tick = async () => {
      if (abortRef.current) {
        setIsPolling(false);
        return;
      }
      if (++attempts > MAX_POLL_ATTEMPTS) {
        setIsPolling(false);
        toast.error('Timed out waiting for image generation.');
        return;
      }

      try {
        const task = await getTaskMutation.mutateAsync({ params: { path: { id } } }) as TaskStatusResponse;

        if (task.status === 'succeeded') {
          const result = await getResultMutation.mutateAsync({ params: { path: { id } } }) as TaskImageResult;

          // Server returns { image: "data:image/png;base64,..." }
          const src: string | null =
            result?.image ?? result?.images?.[0] ?? null;

          if (src) {
            setImages(prev => [{ src, prompt: capturedPrompt, size: capturedSize }, ...prev]);
            toast.success('Image generated!');
          } else {
            toast.error('Generation succeeded but image data was empty.');
          }
          setIsPolling(false);
        } else if (task.status === 'failed' || task.status === 'cancelled') {
          toast.error(`Generation ${task.status}.`);
          setIsPolling(false);
        } else {
          timerRef.current = setTimeout(tick, POLL_INTERVAL_MS);
        }
      } catch (err: any) {
        toast.error('Error while checking task status.', {
          description: err?.message ?? err?.error ?? String(err),
        });
        setIsPolling(false);
      }
    };

    timerRef.current = setTimeout(tick, POLL_INTERVAL_MS);
  }, [getTaskMutation, getResultMutation]);

  const handleGenerate = async () => {
    const trimmed = prompt.trim();
    if (!trimmed) {
      toast.error('Please enter a prompt.');
      return;
    }

    setIsSubmitting(true);
    try {
      const resp = await generateMutation.mutateAsync({
        body: {
          model: 'stable-diffusion',
          prompt: trimmed,
          n: parseInt(numImages, 10),
          size,
        },
      }) as { task_id: string };

      setTaskId(resp.task_id);
      setIsSubmitting(false);
      pollTaskStatus(resp.task_id, trimmed, size);
    } catch (err: any) {
      setIsSubmitting(false);
      toast.error('Failed to submit generation request.', {
        description: err?.message ?? err?.error ?? 'Unknown error',
      });
    }
  };

  const handleDownload = (src: string, index: number) => {
    const a = document.createElement('a');
    a.href = src;
    a.download = `slab-image-${index + 1}.png`;
    a.click();
  };

  const handleCancel = async () => {
    // Stop client-side polling immediately.
    abortRef.current = true;
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    setIsPolling(false);

    // Also cancel the server-side task if we have an ID.
    if (taskId) {
      try {
        await cancelTaskMutation.mutateAsync({ params: { path: { id: taskId } } });
        toast.info('Generation cancelled.');
      } catch (err: any) {
        // Task may have already completed — that's fine.
        toast.info('Stopped polling (server task may have already finished).', {
          description: err?.message ?? err?.error ?? undefined,
        });
      }
    } else {
      toast.info('Generation cancelled.');
    }
  };

  return (
    <>
      {/* ── Lightbox ─────────────────────────────────────────────────────── */}
      <Dialog open={!!lightboxSrc} onOpenChange={() => setLightboxSrc(null)}>
        <DialogContent className="max-w-5xl p-2 bg-black border-neutral-800">
          {lightboxSrc && (
            <img
              src={lightboxSrc}
              alt="Full size"
              className="w-full h-full object-contain rounded"
              style={{ maxHeight: '85vh' }}
            />
          )}
        </DialogContent>
      </Dialog>

      {/* ── Page ─────────────────────────────────────────────────────────── */}
      <div className="h-full bg-background">
        <div className="grid h-full grid-cols-1 gap-0 lg:grid-cols-[380px_1fr]">
          {/* ── Left Panel: Controls ────────────────────────────────────── */}
          <div className="border-r border-border flex flex-col p-5 gap-5 overflow-y-auto">
            {/* Prompt */}
            <div className="space-y-2">
              <Label htmlFor="prompt" className="text-sm font-medium">
                Prompt
              </Label>
              <Textarea
                id="prompt"
                placeholder="A photorealistic landscape at golden hour, dramatic lighting, 8k…"
                value={prompt}
                onChange={e => setPrompt(e.target.value)}
                disabled={isBusy}
                rows={5}
                className="resize-none text-sm"
              />
              <p className="text-xs text-muted-foreground text-right">{prompt.length} chars</p>
            </div>

            {/* Size */}
            <div className="space-y-2">
              <Label htmlFor="size" className="text-sm font-medium">Size</Label>
              <Select value={size} onValueChange={setSize} disabled={isBusy}>
                <SelectTrigger id="size" className="text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SIZES.map(s => (
                    <SelectItem key={s.value} value={s.value} className="text-sm">
                      {s.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Count */}
            <div className="space-y-2">
              <Label htmlFor="count" className="text-sm font-medium">Number of Images</Label>
              <Select value={numImages} onValueChange={setNumImages} disabled={isBusy}>
                <SelectTrigger id="count" className="text-sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {COUNTS.map(c => (
                    <SelectItem key={c} value={c} className="text-sm">{c}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {/* Status info */}
            {isPolling && taskId && (
              <div className="rounded-lg border border-border bg-muted/40 p-3 space-y-1">
                <p className="text-xs font-medium text-foreground flex items-center gap-1.5">
                  <Loader2 className="w-3 h-3 animate-spin" />
                  Generating…
                </p>
                <p className="text-xs text-muted-foreground font-mono truncate">
                  Task {taskId}
                </p>
              </div>
            )}

            <div className="flex gap-2 mt-auto pt-2">
              <Button
                onClick={handleGenerate}
                disabled={!prompt.trim() || isBusy}
                className="flex-1"
              >
                {isSubmitting ? (
                  <><Loader2 className="w-4 h-4 mr-2 animate-spin" />Submitting…</>
                ) : isPolling ? (
                  <><Loader2 className="w-4 h-4 mr-2 animate-spin" />Generating…</>
                ) : (
                  <><Sparkles className="w-4 h-4 mr-2" />Generate</>
                )}
              </Button>
              {isPolling && (
                <Button variant="outline" size="icon" onClick={handleCancel} title="Cancel">
                  <X className="w-4 h-4" />
                </Button>
              )}
            </div>
          </div>

          {/* ── Right Panel: Gallery ────────────────────────────────────── */}
          <div className="overflow-y-auto p-5">
            {images.length === 0 ? (
              <div className="h-full flex flex-col items-center justify-center gap-3 text-muted-foreground select-none">
                <div className="rounded-2xl border border-dashed border-border p-10 flex flex-col items-center gap-3">
                  <ImageIcon className="w-10 h-10 opacity-20" />
                  <p className="text-sm font-medium opacity-50">Generated images will appear here</p>
                </div>
              </div>
            ) : (
              <div className="columns-1 sm:columns-2 xl:columns-3 gap-3 space-y-3">
                {images.map((img, i) => (
                  <div
                    key={img.src}
                    className="break-inside-avoid rounded-xl overflow-hidden border border-border group relative bg-muted/20"
                  >
                    <img
                      src={img.src}
                      alt={`Generated image ${i + 1}`}
                      className="w-full h-auto block"
                      loading="lazy"
                    />
                    {/* Overlay actions */}
                    <div className="absolute inset-0 bg-black/0 group-hover:bg-black/50 transition-colors flex items-center justify-center gap-2 opacity-0 group-hover:opacity-100">
                      <Button
                        size="sm"
                        variant="secondary"
                        className="h-8 text-xs"
                        onClick={() => setLightboxSrc(img.src)}
                      >
                        <ZoomIn className="w-3.5 h-3.5 mr-1" />
                        View
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        className="h-8 text-xs"
                        onClick={() => handleDownload(img.src, i)}
                      >
                        <Download className="w-3.5 h-3.5 mr-1" />
                        Save
                      </Button>
                    </div>
                    {/* Caption */}
                    <div className="px-3 py-2 border-t border-border bg-background/80">
                      <p className="text-xs text-muted-foreground truncate" title={img.prompt}>
                        {img.prompt}
                      </p>
                      <p className="text-xs text-muted-foreground/50 mt-0.5">{img.size}</p>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </>
  );
}
