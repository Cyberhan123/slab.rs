import {
  ChevronDown,
  ChevronUp,
  Download,
  Film,
  History,
  ImagePlus,
  Loader2,
  Maximize2,
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
  type VideoGenerationTask,
} from '@/lib/media-task-api';
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
          <aside className="flex h-full min-h-[520px] flex-col rounded-[28px] border border-border/50 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-soft)_96%,transparent),color-mix(in_oklab,var(--surface-1)_96%,transparent))] p-6 shadow-[0_20px_50px_-38px_color-mix(in_oklab,var(--foreground)_35%,transparent)] lg:min-h-0 lg:overflow-hidden">
            <div className="pb-6">
              <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-muted-foreground">
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
                  placeholder={t('pages.video.workbench.prompt.placeholder')}
                  rows={4}
                  value={prompt}
                  onChange={(event) => setPrompt(event.target.value)}
                  className="min-h-[112px] rounded-[22px] border-border/50 bg-[var(--shell-card)]/78 px-4 py-4 text-[15px] leading-6 text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)] placeholder:text-muted-foreground/70"
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
                  className="h-14 rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-[15px] text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)] placeholder:text-muted-foreground/70"
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
                      className="h-14 w-full rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-base font-semibold text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                      className="h-14 w-full rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-base font-semibold text-foreground shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                  className="group flex w-full flex-col items-center justify-center gap-4 rounded-[22px] border-2 border-dashed border-border/60 bg-[var(--shell-card)]/52 px-5 py-7 text-center transition hover:border-[var(--brand-teal)]/45 hover:bg-[var(--shell-card)]/72"
                >
                  {initImageDataUri ? (
                    <div className="relative w-full overflow-hidden rounded-[18px] border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/80 shadow-[0_18px_30px_-24px_color-mix(in_oklab,var(--foreground)_35%,transparent)]">
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
                      <div className="flex size-14 items-center justify-center rounded-full bg-[var(--shell-card)] text-muted-foreground shadow-[0_18px_30px_-24px_color-mix(in_oklab,var(--foreground)_35%,transparent)]">
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
                  className="hidden"
                  onChange={handleInitImageChange}
                />
              </div>

              <Collapsible open={advancedOpen} onOpenChange={setAdvancedOpen}>
                <CollapsibleTrigger asChild>
                  <button
                    type="button"
                    className="flex w-full items-center justify-between rounded-[18px] border border-border/50 bg-[var(--shell-card)]/72 px-4 py-3 text-sm font-semibold text-foreground/80 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_85%,transparent)] transition hover:border-border/70 hover:text-foreground"
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
                        className="h-12 rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
                      />
                    </div>

                    <div className="space-y-2.5">
                      <FieldLabel>{t('pages.video.workbench.fields.sampler')}</FieldLabel>
                      <Select value={sampleMethod} onValueChange={setSampleMethod}>
                        <SelectTrigger
                          variant="soft"
                          className="h-12 w-full rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                          className="h-12 w-full rounded-[18px] border-border/50 bg-[var(--shell-card)]/78 px-4 text-sm font-medium shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_90%,transparent)]"
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
                className="h-[68px] w-full rounded-[18px] text-base font-semibold shadow-[0_24px_40px_-18px_color-mix(in_oklab,var(--brand-teal)_58%,transparent)]"
                onClick={handleSubmit}
                disabled={isGenerating || !prompt.trim() || !hasSelectedModel}
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
                >
                  {t('pages.video.workbench.actions.cancel')}
                </Button>
              ) : null}
            </div>
          </aside>

          <section className="flex min-h-[520px] flex-col gap-6 lg:min-h-0">
            <div
              className={cn(
                'relative flex min-h-[420px] flex-1 items-center justify-center overflow-hidden rounded-[32px] border border-border/50 bg-[var(--surface-soft)] p-6 shadow-[0_32px_80px_-56px_color-mix(in_oklab,var(--foreground)_45%,transparent)] lg:min-h-0',
              )}
              style={{
                backgroundImage:
                  'radial-gradient(circle at center, color-mix(in oklab,var(--brand-teal) 12%,transparent) 0%, transparent 24%), linear-gradient(135deg, color-mix(in oklab,var(--shell-card) 88%,transparent) 0%, color-mix(in oklab,var(--surface-soft) 92%,transparent) 40%, color-mix(in oklab,var(--shell-card) 90%,transparent) 100%)',
              }}
            >
              <div className="absolute inset-0 opacity-70 [background:radial-gradient(circle_at_top_right,color-mix(in oklab,var(--foreground) 6%,transparent),transparent_38%),radial-gradient(circle_at_bottom_left,color-mix(in oklab,var(--shell-card) 88%,transparent),transparent_34%)]" />

              {videoPath ? (
                <div className="relative z-10 w-full max-w-[640px] space-y-4">
                  <div className="overflow-hidden rounded-[28px] border border-[var(--shell-card)]/50 bg-[var(--media-canvas)]/88 shadow-[0_32px_80px_-42px_color-mix(in_oklab,var(--foreground)_60%,transparent)]">
                    {/* eslint-disable-next-line jsx-a11y/media-has-caption */}
                    <video
                      src={videoPath}
                      controls
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
                    <div className="absolute inset-[-26px] rounded-full bg-[var(--brand-teal)]/18 blur-3xl" />
                    <div className="relative flex size-24 items-center justify-center rounded-[32px] bg-[var(--shell-card)] text-[var(--brand-teal)] shadow-[0_28px_60px_-36px_color-mix(in_oklab,var(--foreground)_45%,transparent)]">
                      {isGenerating ? <Loader2 className="h-10 w-10 animate-spin" /> : <Film className="h-10 w-10" />}
                    </div>
                  </div>

                  <div className="space-y-3">
                    <h2 className="text-[32px] font-semibold tracking-[-0.035em] text-foreground">
                      {stageTitle}
                    </h2>
                    <p className="text-sm leading-7 text-muted-foreground">{stageDescription}</p>
                  </div>
                </div>
              )}

              <div className="absolute bottom-8 left-1/2 z-20 -translate-x-1/2">
                <div className="flex items-center gap-2 rounded-[20px] border border-[var(--shell-card)]/45 bg-[var(--shell-card)]/72 px-4 py-3 backdrop-blur-xl shadow-[0_24px_50px_-34px_color-mix(in_oklab,var(--foreground)_42%,transparent)]">
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

            <div className="rounded-[22px] border border-border/50 bg-[linear-gradient(180deg,color-mix(in_oklab,var(--surface-soft)_95%,transparent),color-mix(in_oklab,var(--surface-1)_92%,transparent))] px-5 py-4 shadow-[0_18px_42px_-34px_color-mix(in_oklab,var(--foreground)_28%,transparent)]">
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

            <div className="rounded-[22px] border border-border/50 bg-[var(--surface-soft)] px-5 py-4 shadow-[0_18px_42px_-34px_color-mix(in_oklab,var(--foreground)_28%,transparent)]">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
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
                    className="rounded-[18px] border border-border/50 bg-[var(--shell-card)] px-4 py-3 text-left transition hover:border-[var(--brand-teal)]/50 hover:shadow-[0_18px_36px_-30px_color-mix(in_oklab,var(--foreground)_38%,transparent)]"
                    onClick={() => void openHistoryDetail(task.task_id)}
                  >
                    <p className="line-clamp-2 text-sm font-semibold leading-5 text-foreground">
                      {task.prompt}
                    </p>
                    <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
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
              <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
                <div className="overflow-hidden rounded-[24px] border border-border/60 bg-[var(--media-canvas)]">
                  {resolveMediaUrl(selectedHistoryTask.video_url) ? (
                    // eslint-disable-next-line jsx-a11y/media-has-caption
                    <video
                      src={resolveMediaUrl(selectedHistoryTask.video_url) ?? undefined}
                      controls
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
                    <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted-foreground">
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
            </>
          ) : null}
        </DialogContent>
      </Dialog>
    </div>
  );
}
