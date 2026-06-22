import {
  ChevronDown,
  ChevronUp,
  Download,
  Film,
  FolderOpen,
  GitCompare,
  History,
  ImagePlus,
  Loader2,
  Maximize2,
  RotateCcw,
  X,
} from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Button } from '@slab/components/button';
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@slab/components/collapsible';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@slab/components/dialog';
import { Input } from '@slab/components/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select';
import { Slider } from '@slab/components/slider';
import { Textarea } from '@slab/components/textarea';
import {
  resolveMediaUrl,
  type GenerationProgress,
  type VideoGenerationTask,
} from '@/lib/media-task-api';
import { GenerationProgressView } from '@/components/generation-progress';
import { cn } from '@/lib/utils';
import { FRAME_OPTIONS, FPS_OPTIONS, SAMPLE_METHODS, SCHEDULERS } from '../const';
import { FieldLabel } from './field-label';
import { ResolutionSliderField } from './resolution-slider-field';
import { SliderField } from './slider-field';
import { StatusMetric } from './status-metric';
import { ToolbarIconButton } from './toolbar-icon-button';

export type VideoWorkbenchProps = {
  advancedOpen: boolean;
  cfgScale: number;
  comparisonTasks: VideoGenerationTask[];
  footerHint: string;
  fps: number;
  frames: number;
  guidance: number;
  generationProgress: GenerationProgress | null;
  handleCancel: () => void | Promise<void>;
  handleDownload: () => void;
  handleInitImageChange: (event: React.ChangeEvent<HTMLInputElement>) => void | Promise<void>;
  handleInitImageDrop: (event: React.DragEvent<HTMLButtonElement>) => void | Promise<void>;
  handleSubmit: () => void | Promise<void>;
  heightStr: string;
  heightValue: number;
  hasSelectedModel: boolean;
  history: VideoGenerationTask[];
  historyDialogOpen: boolean;
  historyError: string | null;
  historyLoading: boolean;
  immersivePreview: boolean;
  initImageDataUri: string | null;
  initImageInputRef: React.RefObject<HTMLInputElement | null>;
  isGenerating: boolean;
  negativePrompt: string;
  prompt: string;
  sampleMethod: string;
  scheduler: string;
  seed: number;
  selectedHistoryTask: VideoGenerationTask | null;
  setAdvancedOpen: (open: boolean) => void;
  setCfgScale: (value: number) => void;
  setFps: (value: number) => void;
  setFrames: (value: number) => void;
  setGuidance: (value: number) => void;
  setHeightStr: (value: string) => void;
  setHistoryDialogOpen: (open: boolean) => void;
  setImmersivePreview: React.Dispatch<React.SetStateAction<boolean>>;
  setInitImageDataUri: (value: string | null) => void;
  setNegativePrompt: (value: string) => void;
  setPrompt: (value: string) => void;
  setSampleMethod: (value: string) => void;
  setScheduler: (value: string) => void;
  setSelectedHistoryTask: (task: VideoGenerationTask | null) => void;
  setSeed: (value: number) => void;
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
  openHistoryDetail: (taskId: string) => void | Promise<void>;
  openHistoryVideoInWorkspace: (task: VideoGenerationTask) => void;
  refillFromHistory: (task: VideoGenerationTask) => void;
  toggleHistoryComparison: (task: VideoGenerationTask) => void;
};

function formatHistoryTime(value: string) {
  return new Date(value).toLocaleString(undefined, {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function VideoWorkbench({
  advancedOpen,
  cfgScale,
  comparisonTasks,
  footerHint,
  fps,
  frames,
  guidance,
  generationProgress,
  handleCancel,
  handleDownload,
  handleInitImageChange,
  handleInitImageDrop,
  handleSubmit,
  heightStr,
  heightValue,
  hasSelectedModel,
  history,
  historyDialogOpen,
  historyError,
  historyLoading,
  immersivePreview,
  initImageDataUri,
  initImageInputRef,
  isGenerating,
  negativePrompt,
  prompt,
  sampleMethod,
  scheduler,
  seed,
  selectedHistoryTask,
  setAdvancedOpen,
  setCfgScale,
  setFps,
  setFrames,
  setGuidance,
  setHeightStr,
  setHistoryDialogOpen,
  setImmersivePreview,
  setInitImageDataUri,
  setNegativePrompt,
  setPrompt,
  setSampleMethod,
  setScheduler,
  setSelectedHistoryTask,
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
  openHistoryDetail,
  openHistoryVideoInWorkspace,
  refillFromHistory,
  toggleHistoryComparison,
}: VideoWorkbenchProps) {
  const { t } = useTranslation();
  const sampleMethodOptions = SAMPLE_METHODS.map((method) =>
    Object.assign({}, method, { label: t(`pages.video.options.sampleMethods.${method.value}`) }),
  );
  const schedulerOptions = SCHEDULERS.map((schedulerItem) =>
    Object.assign({}, schedulerItem, { label: t(`pages.video.options.schedulers.${schedulerItem.value}`) }),
  );

  return (
    <div className="h-full w-full overflow-y-auto bg-[var(--shell-card)] lg:overflow-hidden">
      <div className="mx-auto flex min-h-full w-full max-w-[1200px] flex-col px-4 py-4 sm:px-6 lg:h-full lg:min-h-0 lg:py-5 xl:py-6">
        <div className="grid min-h-0 flex-1 gap-6 lg:grid-cols-[340px_minmax(0,1fr)] xl:grid-cols-[378px_minmax(0,1fr)]">
          <aside className="flex h-full min-h-[520px] flex-col rounded-3xl border border-border/50 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-soft)_96%,transparent),color-mix(in_oklab,var(--surface-1)_96%,transparent))] p-6 lg:min-h-0 lg:overflow-hidden">
            <div className="pb-6">
              <p className="text-caption font-semibold uppercase tracking-eyebrow text-muted-foreground">
                {t('pages.video.workbench.configTitle')}
              </p>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                {t('pages.video.workbench.modelHint')}
              </p>
            </div>

            <div className="min-h-0 flex-1 space-y-6 overflow-y-auto pr-2">
              <div className="space-y-2.5">
                <FieldLabel htmlFor="video-prompt">
                  {t('pages.video.workbench.prompt.label')}
                </FieldLabel>
                <Textarea
                  id="video-prompt"
                  variant="soft"
                  data-testid="video-prompt-input"
                  placeholder={t('pages.video.workbench.prompt.placeholder')}
                  rows={4}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  className="min-h-[112px] rounded-[22px] border-border/50 bg-glass-bg-strong px-4 py-4 text-sm leading-6 text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)] placeholder:text-muted-foreground/70"
                />
              </div>

              <div className="space-y-2.5">
                <FieldLabel htmlFor="video-negative">
                  {t('pages.video.workbench.negativePrompt.label')}
                </FieldLabel>
                <Input
                  id="video-negative"
                  variant="soft"
                  placeholder={t('pages.video.workbench.negativePrompt.placeholder')}
                  value={negativePrompt}
                  onChange={(event) => setNegativePrompt(event.target.value)}
                  className="h-14 rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-sm text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)] placeholder:text-muted-foreground/70"
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <ResolutionSliderField
                  label={t('pages.video.workbench.fields.width')}
                  value={widthStr}
                  min={64}
                  max={2048}
                  step={64}
                  onChange={setWidthStr}
                />
                <ResolutionSliderField
                  label={t('pages.video.workbench.fields.height')}
                  value={heightStr}
                  min={64}
                  max={2048}
                  step={64}
                  onChange={setHeightStr}
                />
              </div>

              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-2.5">
                  <FieldLabel>{t('pages.video.workbench.fields.frames')}</FieldLabel>
                  <Select value={String(frames)} onValueChange={(value) => setFrames(Number(value))}>
                    <SelectTrigger
                      variant="soft"
                      className="h-14 w-full rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-base font-semibold text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                  <FieldLabel>{t('pages.video.workbench.fields.fps')}</FieldLabel>
                  <Select value={String(fps)} onValueChange={(value) => setFps(Number(value))}>
                    <SelectTrigger
                      variant="soft"
                      className="h-14 w-full rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-base font-semibold text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                <FieldLabel>{t('pages.video.workbench.fields.referenceImage')}</FieldLabel>
                <button
                  type="button"
                  onClick={() => initImageInputRef.current?.click()}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={handleInitImageDrop}
                  className="group flex w-full flex-col items-center justify-center gap-4 rounded-[22px] border-2 border-dashed border-border/60 bg-glass-bg px-5 py-7 text-center transition hover:border-[var(--brand-teal)]/45 hover:bg-glass-bg-strong"
                >
                  {initImageDataUri ? (
                    <div className="relative w-full overflow-hidden rounded-[18px] border border-[var(--shell-card)]/70 bg-glass-bg-strong">
                      <img
                        src={initImageDataUri}
                        alt={t('pages.video.workbench.referenceImage.previewAlt')}
                        className="h-36 w-full object-cover"
                      />
                      <div className="flex items-center justify-between gap-3 px-4 py-3 text-left">
                        <div>
                          <p className="text-sm font-semibold text-foreground">
                            {t('pages.video.workbench.referenceImage.readyTitle')}
                          </p>
                          <p className="text-xs text-muted-foreground">
                            {t('pages.video.workbench.referenceImage.readyDescription')}
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
                          aria-label={t('pages.video.workbench.referenceImage.removeAria')}
                        >
                          <X className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <div className="flex size-14 items-center justify-center rounded-full bg-[var(--shell-card)] text-muted-foreground">
                        <ImagePlus className="h-6 w-6" />
                      </div>
                      <div className="space-y-1">
                        <p className="text-sm font-medium text-foreground/80">
                          {t('pages.video.workbench.referenceImage.uploadTitle')}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {t('pages.video.workbench.referenceImage.uploadDescription')}
                        </p>
                      </div>
                    </>
                  )}
                </button>
                <input
                  ref={initImageInputRef}
                  type="file"
                  accept="image/png,image/jpeg"
                  aria-label={t('pages.video.workbench.fields.referenceImage')}
                  className="hidden"
                  onChange={handleInitImageChange}
                />
              </div>

              <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                <CollapsibleTrigger asChild>
                  <button
                    type="button"
                    className="flex w-full items-center justify-between rounded-[18px] border border-border/50 bg-glass-bg-strong px-4 py-3 text-sm font-semibold text-foreground/80 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_85%,transparent)] transition hover:border-border/70 hover:text-foreground"
                  >
                    {t('pages.video.workbench.fields.advanced')}
                    {advancedOpen ? <ChevronUp className="h-4 w-4" /> : <ChevronDown className="h-4 w-4" />}
                  </button>
                </CollapsibleTrigger>

                <CollapsibleContent className="space-y-5 pt-4">
                  <div className="grid gap-4 sm:grid-cols-2">
                    <SliderField
                      label={t('pages.video.workbench.fields.cfgScale')}
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
                      label={t('pages.video.workbench.fields.guidance')}
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
                      label={t('pages.video.workbench.fields.steps')}
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
                        label={t('pages.video.workbench.fields.strength')}
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
                      <FieldLabel>{t('pages.video.workbench.fields.seed')}</FieldLabel>
                      <Input
                        variant="soft"
                        type="number"
                        value={seed}
                        onChange={(event) => setSeed(Number.parseInt(event.target.value, 10))}
                        className="h-12 rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
                      />
                    </div>

                    <div className="space-y-2.5">
                      <FieldLabel>{t('pages.video.workbench.fields.sampler')}</FieldLabel>
                      <Select value={sampleMethod} onValueChange={setSampleMethod}>
                        <SelectTrigger
                          variant="soft"
                          className="h-12 w-full rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent variant="soft">
                          {sampleMethodOptions.map((method) => (
                            <SelectItem key={method.value} value={method.value}>
                              {method.label}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="space-y-2.5 sm:col-span-2">
                      <FieldLabel>{t('pages.video.workbench.fields.scheduler')}</FieldLabel>
                      <Select value={scheduler} onValueChange={setScheduler}>
                        <SelectTrigger
                          variant="soft"
                          className="h-12 w-full rounded-[18px] border-border/50 bg-glass-bg-strong px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
                        >
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent variant="soft">
                          {schedulerOptions.map((schedulerItem) => (
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
                className="h-[68px] w-full rounded-[18px] text-base font-semibold shadow-elevation-2"
                onClick={handleSubmit}
                disabled={isGenerating || !prompt.trim() || !hasSelectedModel}
                data-testid="video-generate-button"
              >
                {isGenerating ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t('pages.video.workbench.actions.generating')}
                  </>
                ) : (
                  <>
                    <Film className="h-4 w-4" />
                    {t('pages.video.workbench.actions.generate')}
                  </>
                )}
              </Button>
              {isGenerating ? (
                <Button
                  variant="pill"
                  size="pill"
                  className="h-11 w-full rounded-[16px]"
                  onClick={handleCancel}
                  data-testid="video-cancel-button"
                >
                  {t('pages.video.workbench.actions.cancel')}
                </Button>
              ) : null}
            </div>
          </aside>

          <section className="flex min-h-[520px] flex-col gap-6 lg:min-h-0">
            <div
              className={cn(
                'relative flex min-h-[420px] flex-1 items-center justify-center overflow-hidden rounded-3xl border border-border/50 bg-[var(--surface-soft)] p-6 lg:min-h-0',
              )}
              style={{
                backgroundImage:
                  'radial-gradient(circle at center, color-mix(in oklab,var(--brand-teal) 12%,transparent) 0%, transparent 24%), linear-gradient(135deg, color-mix(in oklab,var(--shell-card) 88%,transparent) 0%, color-mix(in oklab,var(--surface-soft) 92%,transparent) 40%, color-mix(in oklab,var(--shell-card) 90%,transparent) 100%)',
              }}
            >
              <div className="absolute inset-0 opacity-70 [background:radial-gradient(circle_at_top_right,color-mix(in oklab,var(--foreground) 6%,transparent),transparent_38%),radial-gradient(circle_at_bottom_left,color-mix(in oklab,var(--shell-card) 88%,transparent),transparent_34%)]" />

              {videoPath ? (
                <div className="relative z-10 w-full max-w-[640px] space-y-4">
                  <div className="overflow-hidden rounded-3xl border border-[var(--shell-card)]/50 bg-[color:color-mix(in_oklab,var(--media-canvas)_88%,transparent)]">
                    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
                    <video
                      src={videoPath}
                      controls
                      aria-label={t('pages.video.workbench.stage.renderStatus')}
                      autoPlay
                      loop
                      className={cn(
                        'w-full bg-[var(--media-canvas)]',
                        immersivePreview ? 'h-[520px] object-cover' : 'max-h-[520px] object-contain',
                      )}
                    />
                  </div>
                </div>
              ) : (
                <div className="relative z-10 flex max-w-[340px] flex-col items-center gap-6 text-center">
                  <div className="relative">
                    <div className="absolute inset-[-26px] rounded-full bg-[color:color-mix(in_oklab,var(--brand-teal)_18%,transparent)] blur-3xl" />
                    <div className="relative flex size-24 items-center justify-center rounded-3xl bg-[var(--shell-card)] text-[color:var(--brand-teal)]">
                      {isGenerating ? <Loader2 className="h-10 w-10 animate-spin" /> : <Film className="h-10 w-10" />}
                    </div>
                  </div>

                  <div className="space-y-3">
                    <h2 className="text-3xl font-semibold tracking-display text-foreground">
                      {stageTitle}
                    </h2>
                    <p className="text-sm leading-7 text-muted-foreground">{stageDescription}</p>
                  </div>
                  {isGenerating ? (
                    <GenerationProgressView
                      progress={generationProgress}
                      labels={{
                        eta: t('pages.video.progress.eta'),
                        finalizing: t('pages.video.progress.finalizing'),
                        queued: t('pages.video.progress.queued'),
                        running: t('pages.video.progress.running'),
                        step: t('pages.video.progress.step'),
                        title: t('pages.video.progress.title'),
                      }}
                      className="w-full max-w-[360px]"
                      testId="video-generation-progress"
                    />
                  ) : null}
                </div>
              )}

              <div className="absolute bottom-8 left-1/2 z-20 -translate-x-1/2">
                <div className="flex items-center gap-2 rounded-[20px] border border-[var(--shell-card)]/45 bg-glass-bg-strong px-4 py-3 backdrop-blur-xl">
                  <ToolbarIconButton
                    icon={Maximize2}
                    label={t('pages.video.workbench.stage.toggleScale')}
                    active={immersivePreview}
                    onClick={() => setImmersivePreview((current) => !current)}
                  />
                  <ToolbarIconButton
                    icon={Download}
                    label={t('pages.video.workbench.stage.downloadVideo')}
                    disabled={!videoPath}
                    onClick={handleDownload}
                  />
                </div>
              </div>
            </div>

            <div className="rounded-[22px] border border-border/50 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-soft)_95%,transparent),color-mix(in_oklab,var(--surface-1)_92%,transparent))] px-5 py-4">
              <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
                <div className="grid gap-4 sm:grid-cols-3">
                  <StatusMetric
                    label={t('pages.video.workbench.stage.renderStatus')}
                    value={stageStatus}
                  />
                  <StatusMetric
                    label={t('pages.video.workbench.stage.clipSpec')}
                    value={t('pages.video.workbench.stage.clipSpecValue', { frames, fps })}
                  />
                  <StatusMetric
                    label={t('pages.video.workbench.stage.canvas')}
                    value={`${widthValue} x ${heightValue}`}
                  />
                </div>
                <p className="text-xs font-medium text-muted-foreground xl:text-right">{footerHint}</p>
              </div>
            </div>

            <div className="rounded-[22px] border border-border/50 bg-[var(--surface-soft)] px-5 py-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-caption font-semibold uppercase tracking-eyebrow text-muted-foreground">
                    {t('pages.video.history.title')}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {historyLoading
                      ? t('pages.video.history.loading')
                      : historyError
                        ? t('pages.video.history.error', { message: historyError })
                        : t('pages.video.history.description')}
                  </p>
                </div>
                <History className="h-4 w-4 text-muted-foreground" />
              </div>
              <div className="mt-3 grid gap-3 lg:grid-cols-2">
                {history.slice(0, 4).map((task) => (
                  <button
                    key={task.task_id}
                    type="button"
                    data-testid={`video-history-item-${task.task_id}`}
                    className="rounded-[18px] border border-border/50 bg-[var(--shell-card)] px-4 py-3 text-left transition hover:border-[var(--brand-teal)]/50"
                    onClick={() => void openHistoryDetail(task.task_id)}
                  >
                    <p className="line-clamp-2 text-sm font-semibold leading-5 text-foreground">
                      {task.prompt}
                    </p>
                    <div className="mt-2 flex flex-wrap items-center gap-2 text-caption text-muted-foreground">
                      <span className="rounded-full bg-[var(--surface-soft)] px-2 py-0.5">
                        {task.status}
                      </span>
                      <span>{task.frames}f / {task.fps}fps</span>
                      <span>{formatHistoryTime(task.created_at)}</span>
                    </div>
                  </button>
                ))}
                {!historyLoading && history.length === 0 ? (
                  <div className="rounded-[18px] border border-dashed border-border/60 bg-[var(--shell-card)] px-4 py-5 text-sm text-muted-foreground lg:col-span-2">
                    {t('pages.video.history.empty')}
                  </div>
                ) : null}
              </div>
            </div>
          </section>
        </div>
      </div>

      <Dialog
        open={historyDialogOpen}
        onOpenChange={(open) => {
          setHistoryDialogOpen(open);
          if (!open) {
            setSelectedHistoryTask(null);
          }
        }}
      >
        <DialogContent className="max-w-4xl">
          {selectedHistoryTask ? (
            <>
              <DialogHeader>
                <DialogTitle>{t('pages.video.history.detailTitle')}</DialogTitle>
                <DialogDescription>
                  {selectedHistoryTask.status} | {selectedHistoryTask.frames} frames at {selectedHistoryTask.fps} fps
                </DialogDescription>
              </DialogHeader>
              <div className="flex flex-wrap justify-end gap-2">
                <Button
                  type="button"
                  variant="pill"
                  size="sm"
                  data-testid="video-history-refill"
                  onClick={() => refillFromHistory(selectedHistoryTask)}
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  {t('pages.video.history.actions.refill')}
                </Button>
                <Button
                  type="button"
                  variant="pill"
                  size="sm"
                  data-testid="video-history-open-workspace"
                  onClick={() => openHistoryVideoInWorkspace(selectedHistoryTask)}
                >
                  <FolderOpen className="h-3.5 w-3.5" />
                  {t('pages.video.history.actions.openWorkspace')}
                </Button>
                <Button
                  type="button"
                  variant="pill"
                  size="sm"
                  data-testid="video-history-compare-toggle"
                  onClick={() => toggleHistoryComparison(selectedHistoryTask)}
                >
                  <GitCompare className="h-3.5 w-3.5" />
                  {comparisonTasks.some((task) => task.task_id === selectedHistoryTask.task_id)
                    ? t('pages.video.history.actions.removeCompare')
                    : t('pages.video.history.actions.compare')}
                </Button>
              </div>
              <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
                <div className="overflow-hidden rounded-2xl border border-border/60 bg-[var(--media-canvas)]">
                  {resolveMediaUrl(selectedHistoryTask.video_url) ? (
                    // eslint-disable-next-line jsx-a11y/media-has-caption
                    <video
                      src={resolveMediaUrl(selectedHistoryTask.video_url) ?? undefined}
                      controls
                      aria-label={t('pages.video.history.detailTitle')}
                      className="max-h-[62vh] w-full bg-[var(--media-canvas)] object-contain"
                    />
                  ) : (
                    <div className="flex min-h-[260px] items-center justify-center text-sm text-muted-foreground">
                      {t('pages.video.history.noArtifact')}
                    </div>
                  )}
                </div>
                <div className="space-y-4 rounded-[22px] bg-[var(--surface-soft)] p-4">
                  <div>
                    <p className="text-caption font-bold uppercase tracking-eyebrow text-muted-foreground">
                      {t('pages.video.workbench.prompt.label')}
                    </p>
                    <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-foreground">
                      {selectedHistoryTask.prompt}
                    </p>
                  </div>
                  <div className="grid grid-cols-2 gap-3 text-sm">
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.video.history.fields.size')}</p>
                      <p className="font-semibold">{selectedHistoryTask.width} x {selectedHistoryTask.height}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.video.history.fields.clip')}</p>
                      <p className="font-semibold">{selectedHistoryTask.frames} / {selectedHistoryTask.fps}fps</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.video.history.fields.backend')}</p>
                      <p className="truncate font-semibold">{selectedHistoryTask.backend_id}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.video.history.fields.model')}</p>
                      <p className="truncate font-semibold">{selectedHistoryTask.model_id ?? selectedHistoryTask.model_path}</p>
                    </div>
                  </div>
                  {selectedHistoryTask.error_msg ? (
                    <p className="rounded-xl bg-destructive/10 p-3 text-xs leading-5 text-destructive">
                      {selectedHistoryTask.error_msg}
                    </p>
                  ) : null}
                </div>
              </div>
              {comparisonTasks.length > 0 ? (
                <VideoComparisonPanel tasks={comparisonTasks} />
              ) : null}
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </div>
  );
}

const VIDEO_COMPARE_FIELDS = [
  'prompt',
  'negative_prompt',
  'width',
  'height',
  'video_frames',
  'fps',
  'cfg_scale',
  'guidance',
  'steps',
  'seed',
  'sample_method',
  'scheduler',
  'strength',
] as const;

function VideoComparisonPanel({ tasks }: { tasks: VideoGenerationTask[] }) {
  const { t } = useTranslation();
  const [firstTask, secondTask] = tasks;

  return (
    <div
      className="mt-5 grid gap-4 rounded-[22px] border border-border/60 bg-[var(--surface-soft)] p-4 lg:grid-cols-2"
      data-testid="video-history-compare"
    >
      {tasks.map((task, index) => (
        <div key={task.task_id} className="space-y-3">
          <div className="overflow-hidden rounded-[18px] border border-border/60 bg-[var(--media-canvas)]">
            {resolveMediaUrl(task.video_url) ? (
              // eslint-disable-next-line jsx-a11y/media-has-caption
              <video
                src={resolveMediaUrl(task.video_url) ?? undefined}
                controls
                aria-label={t('pages.video.history.compareArtifact', { index: index + 1 })}
                className="max-h-56 w-full object-contain"
              />
            ) : (
              <div className="flex h-36 items-center justify-center text-sm text-muted-foreground">
                {t('pages.video.history.noArtifact')}
              </div>
            )}
          </div>
          <div className="space-y-1 text-xs">
            {VIDEO_COMPARE_FIELDS.map((field) => {
              const value = compareFieldValue(task, field);
              const differs =
                firstTask && secondTask &&
                compareFieldValue(firstTask, field) !== compareFieldValue(secondTask, field);
              return (
                <div
                  key={field}
                  className="grid grid-cols-[108px_minmax(0,1fr)] gap-2 rounded-lg px-2 py-1"
                  data-testid={differs ? 'param-diff' : undefined}
                >
                  <span className="font-medium text-muted-foreground">{field}</span>
                  <span
                    className={cn(
                      'min-w-0 break-words font-mono text-foreground',
                      differs && 'rounded-md bg-[color:color-mix(in_oklab,var(--brand-gold)_18%,transparent)] px-1 text-[color:color-mix(in_oklab,var(--brand-gold)_78%,var(--foreground))]',
                    )}
                  >
                    {value || '-'}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

function compareFieldValue(
  task: VideoGenerationTask,
  field: (typeof VIDEO_COMPARE_FIELDS)[number],
) {
  const value = field === 'video_frames' ? task.request_data.video_frames : task.request_data[field];
  return value === null || value === undefined ? '' : String(value);
}
