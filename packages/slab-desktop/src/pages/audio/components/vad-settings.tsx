import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@slab/components/select';
import { Switch } from '@slab/components/switch';
import { useTranslation } from '@slab/i18n';
import type { CatalogModel } from '@slab/api/models';

const BUNDLED_VAD_MODEL_ID = '__bundled_vad__';

export type VadSettingsProps = {
  bundledVadLabel: string;
  enableVad: boolean;
  hasBundledVad: boolean;
  setEnableVad: (value: boolean) => void;
  isTauri: boolean;
  isBusy: boolean;
  isUsingBundledVad: boolean;
  selectedVadModelId: string;
  setSelectedVadModelId: (value: string) => void;
  catalogModelsLoading: boolean;
  whisperVadModels: CatalogModel[];
  selectedVadModel: CatalogModel | undefined;
  vadThreshold: string;
  setVadThreshold: (value: string) => void;
  vadMinSpeechDurationMs: string;
  setVadMinSpeechDurationMs: (value: string) => void;
  vadMinSilenceDurationMs: string;
  setVadMinSilenceDurationMs: (value: string) => void;
  vadMaxSpeechDurationS: string;
  setVadMaxSpeechDurationS: (value: string) => void;
  vadSpeechPadMs: string;
  setVadSpeechPadMs: (value: string) => void;
  vadSamplesOverlap: string;
  setVadSamplesOverlap: (value: string) => void;
};

export function VadSettings({
  bundledVadLabel,
  enableVad,
  hasBundledVad,
  setEnableVad,
  isTauri,
  isBusy,
  isUsingBundledVad,
  selectedVadModelId,
  setSelectedVadModelId,
  catalogModelsLoading,
  whisperVadModels,
  selectedVadModel,
  vadThreshold,
  setVadThreshold,
  vadMinSpeechDurationMs,
  setVadMinSpeechDurationMs,
  vadMinSilenceDurationMs,
  setVadMinSilenceDurationMs,
  vadMaxSpeechDurationS,
  setVadMaxSpeechDurationS,
  vadSpeechPadMs,
  setVadSpeechPadMs,
  vadSamplesOverlap,
  setVadSamplesOverlap,
}: VadSettingsProps) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[22px] border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/60 p-4 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_70%,transparent)]">
      <div className="flex items-start justify-between gap-5">
        <div className="space-y-1">
          <Label htmlFor="enable-vad" className="text-base font-semibold text-foreground">
            {t('pages.audio.vad.title')}
          </Label>
          <p className="text-sm leading-5 text-muted-foreground">
            {t('pages.audio.vad.description')}
          </p>
        </div>
        <Switch
          id="enable-vad"
          checked={enableVad}
          onCheckedChange={setEnableVad}
          disabled={!isTauri || isBusy}
        />
      </div>

      {enableVad && (
        <div className="mt-4 space-y-4 border-t border-border/60 pt-4">
          <div className="space-y-2">
            <Label className="text-[12px] font-semibold text-foreground">
              {t('pages.audio.vad.modelLabel')}
            </Label>
            <Select
              value={selectedVadModelId}
              onValueChange={setSelectedVadModelId}
              disabled={!isTauri || isBusy || (!hasBundledVad && whisperVadModels.length === 0)}
            >
              <SelectTrigger
                variant="soft"
                size="pill"
                className="w-full justify-between border-border/70 bg-[var(--shell-card)] shadow-none"
              >
                <SelectValue
                  placeholder={
                    catalogModelsLoading
                      ? t('pages.audio.vad.loadingModels')
                      : hasBundledVad
                        ? t('pages.audio.vad.useBundled')
                        : t('pages.audio.vad.selectModel')
                  }
                />
              </SelectTrigger>
              <SelectContent variant="soft">
                {hasBundledVad && (
                  <SelectItem value={BUNDLED_VAD_MODEL_ID}>{bundledVadLabel}</SelectItem>
                )}
                {!hasBundledVad && whisperVadModels.length === 0 ? (
                  <div className="px-2 py-1.5 text-sm text-muted-foreground">
                    {t('pages.audio.vad.noDedicatedModels')}
                  </div>
                ) : (
                  whisperVadModels.map((model) => (
                    <SelectItem key={model.id} value={model.id}>
                      {model.display_name}
                    </SelectItem>
                  ))
                )}
              </SelectContent>
            </Select>
            <p className="text-xs leading-5 text-muted-foreground">
              {isUsingBundledVad
                ? t('pages.audio.vad.usingBundledDescription')
                : selectedVadModel
                ? selectedVadModel.local_path
                  ? t('pages.audio.vad.localReadyDescription')
                  : t('pages.audio.vad.autoDownloadDescription')
                : hasBundledVad
                  ? t('pages.audio.vad.bundledOrOverrideDescription')
                  : t('pages.audio.vad.chooseDedicatedDescription')}
            </p>
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="vad-threshold" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.threshold')}
              </Label>
              <Input
                id="vad-threshold"
                type="number"
                inputMode="decimal"
                min={0}
                max={1}
                step={0.01}
                value={vadThreshold}
                onChange={(e) => setVadThreshold(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.threshold')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vad-min-speech-duration" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.minSpeech')}
              </Label>
              <Input
                id="vad-min-speech-duration"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={vadMinSpeechDurationMs}
                onChange={(e) => setVadMinSpeechDurationMs(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.minSpeech')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vad-min-silence-duration" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.minSilence')}
              </Label>
              <Input
                id="vad-min-silence-duration"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={vadMinSilenceDurationMs}
                onChange={(e) => setVadMinSilenceDurationMs(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.minSilence')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vad-max-speech-duration" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.maxSpeech')}
              </Label>
              <Input
                id="vad-max-speech-duration"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.1}
                value={vadMaxSpeechDurationS}
                onChange={(e) => setVadMaxSpeechDurationS(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.maxSpeech')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vad-speech-pad" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.speechPad')}
              </Label>
              <Input
                id="vad-speech-pad"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={vadSpeechPadMs}
                onChange={(e) => setVadSpeechPadMs(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.speechPad')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="vad-samples-overlap" className="text-xs font-semibold text-foreground">
                {t('pages.audio.vad.fields.samplesOverlap')}
              </Label>
              <Input
                id="vad-samples-overlap"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.01}
                value={vadSamplesOverlap}
                onChange={(e) => setVadSamplesOverlap(e.target.value)}
                placeholder={t('pages.audio.vad.placeholders.samplesOverlap')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
