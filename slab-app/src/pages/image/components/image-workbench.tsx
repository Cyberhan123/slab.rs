import type { RefObject } from 'react';
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
import { cn } from '@/lib/utils';
import type { ImageModelOption } from '../hooks/use-image-model-preparation';
import {
  DIMENSION_PRESETS,
  SAMPLE_METHODS,
  SCHEDULERS,
  SIDEBAR_INPUT_CLASSNAME,
  SIDEBAR_LABEL_CLASSNAME,
  SIDEBAR_TEXTAREA_CLASSNAME,
  type GeneratedImage,
} from '../const';
import { SliderField } from './slider-field';

type ImageWorkbenchProps = {
  activeDimensionPreset: string | null;
  advancedOpen: boolean;
  catalogLoading: boolean;
  cfgScale: number;
  clipSkip: number;
  eta: number;
  guidance: number;
  handleCancel: () => void;
  handleDimensionPreset: (width: number, height: number) => void;
  handleDownload: (src: string, index: number) => void;
  handleInitImageChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  handleSubmit: () => void;
  heightStr: string;
  images: GeneratedImage[];
  initImageDataUri: string | null;
  initImageInputRef: RefObject<HTMLInputElement | null>;
  isBusy: boolean;
  isGenerating: boolean;
  isPreparingModel: boolean;
  mode: 'txt2img' | 'img2img';
  modelOptions: ImageModelOption[];
  negativePrompt: string;
  numImages: number;
  parsedHeight: number;
  parsedWidth: number;
  prompt: string;
  sampleMethod: string;
  scheduler: string;
  seed: number;
  selectedModelId: string;
  setAdvancedOpen: (open: boolean) => void;
  setCfgScale: (value: number) => void;
  setClipSkip: (value: number) => void;
  setEta: (value: number) => void;
  setGuidance: (value: number) => void;
  setHeightStr: (value: string) => void;
  setInitImageDataUri: (value: string | null) => void;
  setMode: (mode: 'txt2img' | 'img2img') => void;
  setNegativePrompt: (value: string) => void;
  setNumImages: (value: number) => void;
  setPrompt: (value: string) => void;
  setSampleMethod: (value: string) => void;
  setScheduler: (value: string) => void;
  setSeed: (value: number) => void;
  setSelectedModelId: (value: string) => void;
  setSteps: (value: number) => void;
  setStrength: (value: number) => void;
  setWidthStr: (value: string) => void;
  setZoomedImage: (src: string | null) => void;
  steps: number;
  strength: number;
  widthStr: string;
  zoomedImage: string | null;
};

export function ImageWorkbench({
  activeDimensionPreset,
  advancedOpen,
  catalogLoading,
  cfgScale,
  clipSkip,
  eta,
  guidance,
  handleCancel,
  handleDimensionPreset,
  handleDownload,
  handleInitImageChange,
  handleSubmit,
  heightStr,
  images,
  initImageDataUri,
  initImageInputRef,
  isBusy,
  isGenerating,
  isPreparingModel,
  mode,
  modelOptions,
  negativePrompt,
  numImages,
  parsedHeight,
  parsedWidth,
  prompt,
  sampleMethod,
  scheduler,
  seed,
  selectedModelId,
  setAdvancedOpen,
  setCfgScale,
  setClipSkip,
  setEta,
  setGuidance,
  setHeightStr,
  setInitImageDataUri,
  setMode,
  setNegativePrompt,
  setNumImages,
  setPrompt,
  setSampleMethod,
  setScheduler,
  setSeed,
  setSelectedModelId,
  setSteps,
  setStrength,
  setWidthStr,
  setZoomedImage,
  steps,
  strength,
  widthStr,
  zoomedImage,
}: ImageWorkbenchProps) {
  return (
    <div className="h-full w-full overflow-y-auto bg-[var(--shell-card)] xl:overflow-hidden">
      <div className="mx-auto flex min-h-full max-w-[1248px] flex-col px-4 py-4 sm:px-6 xl:h-full xl:min-h-0">
        <SplitWorkbench
          className="h-full min-h-0 gap-6 xl:grid-cols-[320px_minmax(0,1fr)] xl:gap-0"
          sidebarClassName="space-y-0"
          mainClassName="min-h-full xl:min-h-0"
          sidebar={
            <aside className="flex h-full flex-col rounded-[28px] border border-border/60 bg-[var(--surface-soft)] px-5 py-5 xl:min-h-0 xl:overflow-hidden xl:rounded-none xl:border-0 xl:border-r xl:border-border/50 xl:px-6 xl:py-6">
              <div className="flex h-full min-h-0 flex-col">
                <div className="space-y-6 xl:min-h-0 xl:flex-1 xl:overflow-y-auto xl:pr-2">
                  <div className="space-y-4">
                    <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted-foreground">
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
                          className="h-11 rounded-[16px] border border-transparent text-[14px] font-medium text-muted-foreground shadow-none data-[state=active]:border-border/70 data-[state=active]:bg-[var(--shell-card)] data-[state=active]:text-foreground data-[state=active]:shadow-[var(--shell-elevation)]"
                        >
                          Text to Image
                        </TabsTrigger>
                        <TabsTrigger
                          value="img2img"
                          className="h-11 rounded-[16px] border border-transparent text-[14px] font-medium text-muted-foreground shadow-none data-[state=active]:border-border/70 data-[state=active]:bg-[var(--shell-card)] data-[state=active]:text-foreground data-[state=active]:shadow-[var(--shell-elevation)]"
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
                            className="group flex w-full flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border/50 bg-[var(--shell-card)] px-4 py-4 text-center transition hover:border-[var(--brand-teal)]/70 hover:bg-[var(--surface-soft)]"
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
                                  className="absolute top-2 right-2 border-[var(--shell-card)]/80 bg-[var(--shell-card)]/90 shadow-sm"
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
                                <div className="flex size-12 items-center justify-center rounded-[14px] bg-[var(--surface-soft)] text-muted-foreground transition group-hover:bg-[var(--brand-teal)]/10 group-hover:text-[var(--brand-teal)]">
                                  <ImageIcon className="size-5" />
                                </div>
                                <div className="space-y-1">
                                  <p className="text-sm font-medium text-foreground">
                                    Click to choose an image
                                  </p>
                                  <p className="text-xs text-muted-foreground">
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
                      <SelectContent className="rounded-[16px] border-border/70 bg-[var(--shell-card)] shadow-[0_24px_48px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
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
                      <span className="rounded-full bg-[var(--brand-teal)]/15 px-2 py-1 font-mono text-[10px] leading-none text-[var(--brand-teal)]">
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
                              'flex h-10 items-center justify-center rounded-xl border bg-[var(--shell-card)] px-3 text-[11px] font-medium text-foreground transition',
                              isActive
                                ? 'border-[var(--brand-teal)]/60 shadow-[var(--shell-elevation)]'
                                : 'border-border/60 hover:border-border/50',
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
                      <SelectContent className="rounded-[16px] border-border/70 bg-[var(--shell-card)] shadow-[0_24px_48px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
                        {[1, 2, 4].map((count) => (
                          <SelectItem key={count} value={String(count)}>
                            {count} {count === 1 ? 'Image' : 'Images'}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                    <div className="border-t border-border/70">
                      <CollapsibleTrigger asChild>
                        <button
                          type="button"
                          className="flex w-full items-center justify-between py-3 text-left"
                        >
                          <span className="text-xs font-bold text-muted-foreground">
                            Advanced Settings
                          </span>
                          {advancedOpen ? (
                            <ChevronUp className="h-4 w-4 text-muted-foreground" />
                          ) : (
                            <ChevronDown className="h-4 w-4 text-muted-foreground" />
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
                          <SelectContent className="rounded-[16px] border-border/70 bg-[var(--shell-card)] shadow-[0_24px_48px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
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
                          <SelectContent className="rounded-[16px] border-border/70 bg-[var(--shell-card)] shadow-[0_24px_48px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
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

                  <div className="mt-auto shrink-0 pt-8 xl:pt-6">
                    <Button
                      className="h-14 w-full rounded-xl bg-[linear-gradient(135deg,var(--brand-teal)_0%,color-mix(in_oklab,var(--brand-teal)_85%,var(--shell-card))_100%)] text-base font-semibold text-[var(--brand-teal-foreground)] shadow-[0_10px_15px_-3px_color-mix(in_oklab,var(--brand-teal)_20%,transparent),0_4px_6px_-4px_color-mix(in_oklab,var(--brand-teal)_20%,transparent)] hover:brightness-[1.03]"
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
                        className="mt-3 h-10 w-full rounded-xl text-muted-foreground hover:bg-[var(--shell-card)]/60 hover:text-foreground"
                        onClick={handleCancel}
                      >
                        Cancel generation
                      </Button>
                    ) : null}
                  </div>
                </div>
              </div>
            </aside>
          }
          main={
            <section className="h-full min-h-[520px] rounded-[28px] border border-border/60 bg-[var(--shell-card)] xl:min-h-0 xl:overflow-hidden xl:rounded-none xl:border-0">
              {images.length === 0 ? (
                <div className="flex h-full min-h-[520px] items-center justify-center px-6 py-12 xl:min-h-[780px]">
                  <div className="flex max-w-[448px] flex-col items-center gap-6 text-center">
                    <div className="relative flex items-center justify-center">
                      <div className="flex size-32 items-center justify-center rounded-full bg-[var(--surface-soft)] text-muted-foreground/60">
                        <ImageIcon className="size-14 stroke-[1.5]" />
                      </div>
                      <div className="absolute -right-2 -bottom-2 flex size-12 items-center justify-center rounded-[14px] bg-[linear-gradient(135deg,var(--brand-teal)_0%,color-mix(in_oklab,var(--brand-teal)_85%,var(--shell-card))_100%)] text-[var(--brand-teal-foreground)] shadow-[0_10px_15px_-3px_color-mix(in_oklab,var(--brand-teal)_24%,transparent),0_4px_6px_-4px_color-mix(in_oklab,var(--brand-teal)_22%,transparent)]">
                        {isGenerating ? (
                          <Loader2 className="size-5 animate-spin" />
                        ) : (
                          <Sparkles className="size-5" />
                        )}
                      </div>
                    </div>
                    <div className="space-y-2">
                      <h2 className="text-[30px] font-semibold tracking-[-0.03em] text-foreground">
                        {isGenerating ? 'Generating images...' : 'Ready to create?'}
                      </h2>
                      <p className="mx-auto max-w-[320px] text-sm leading-6 text-muted-foreground">
                        {isGenerating
                          ? 'Your task is running. Generated images will appear here automatically.'
                          : 'Enter a prompt and adjust the parameters to see your imagination come to life.'}
                      </p>
                    </div>
                    {isGenerating ? (
                      <div className="rounded-full bg-[var(--surface-soft)] px-3 py-1 text-sm text-muted-foreground">
                        Task running
                      </div>
                    ) : (
                      <div className="flex flex-wrap items-center justify-center gap-4 text-sm text-muted-foreground">
                        <button
                          type="button"
                          className="inline-flex items-center gap-1.5 transition hover:text-foreground"
                        >
                          <History className="size-3.5" />
                          View History
                        </button>
                        <span className="size-1 rounded-full bg-border/30" />
                        <button
                          type="button"
                          className="inline-flex items-center gap-1.5 transition hover:text-foreground"
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
                  <div className="border-b border-border/60 px-6 py-6 xl:px-10">
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div>
                        <h2 className="text-[24px] font-semibold tracking-[-0.02em] text-foreground">
                          Generated Images
                        </h2>
                        <p className="mt-1 text-sm text-muted-foreground">
                          Review the latest renders, zoom in for detail, or download the best take.
                        </p>
                      </div>
                      <span className="rounded-full bg-[var(--brand-teal)]/15 px-3 py-1 text-sm font-medium text-[var(--brand-teal)]">
                        {images.length} {images.length === 1 ? 'image' : 'images'}
                      </span>
                    </div>
                  </div>
                  <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6 xl:px-10 xl:py-8">
                    <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
                      {images.map((image, index) => (
                        <figure
                          key={`${image.src}-${index}`}
                          className="group overflow-hidden rounded-[24px] border border-border/60 bg-[var(--surface-soft)] shadow-[0_18px_32px_-28px_color-mix(in_oklab,var(--foreground)_28%,transparent)]"
                        >
                          <div className="relative overflow-hidden bg-[var(--shell-card)]">
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
                                className="border-[var(--shell-card)]/80 bg-[var(--shell-card)]/95 shadow-sm backdrop-blur"
                                onClick={() => setZoomedImage(image.src)}
                              >
                                <ZoomIn className="h-4 w-4" />
                              </Button>
                              <Button
                                variant="pill"
                                size="icon-sm"
                                className="border-[var(--shell-card)]/80 bg-[var(--shell-card)]/95 shadow-sm backdrop-blur"
                                onClick={() => handleDownload(image.src, index)}
                              >
                                <Download className="h-4 w-4" />
                              </Button>
                            </div>
                          </div>
                          <figcaption className="space-y-3 border-t border-border/48 bg-[var(--shell-card)] px-4 py-4">
                            <p className="line-clamp-2 text-sm leading-6 text-foreground/80">
                              {image.prompt}
                            </p>
                            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                              <Badge variant="chip">{image.mode}</Badge>
                              <span className="rounded-full bg-[var(--surface-soft)] px-2.5 py-1">
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
