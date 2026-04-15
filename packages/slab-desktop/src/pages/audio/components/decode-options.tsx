import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';
import { Switch } from '@slab/components/switch';
import { useTranslation } from '@slab/i18n';

export type DecodeOptionsProps = {
  showDecodeOptions: boolean;
  setShowDecodeOptions: (value: boolean) => void;
  isTauri: boolean;
  isBusy: boolean;
  decodeOffsetMs: string;
  setDecodeOffsetMs: (value: string) => void;
  decodeDurationMs: string;
  setDecodeDurationMs: (value: string) => void;
  decodeWordThold: string;
  setDecodeWordThold: (value: string) => void;
  decodeMaxLen: string;
  setDecodeMaxLen: (value: string) => void;
  decodeMaxTokens: string;
  setDecodeMaxTokens: (value: string) => void;
  decodeTemperature: string;
  setDecodeTemperature: (value: string) => void;
  decodeTemperatureInc: string;
  setDecodeTemperatureInc: (value: string) => void;
  decodeEntropyThold: string;
  setDecodeEntropyThold: (value: string) => void;
  decodeLogprobThold: string;
  setDecodeLogprobThold: (value: string) => void;
  decodeNoSpeechThold: string;
  setDecodeNoSpeechThold: (value: string) => void;
  decodeNoContext: boolean;
  setDecodeNoContext: (value: boolean) => void;
  decodeNoTimestamps: boolean;
  setDecodeNoTimestamps: (value: boolean) => void;
  decodeTokenTimestamps: boolean;
  setDecodeTokenTimestamps: (value: boolean) => void;
  decodeSplitOnWord: boolean;
  setDecodeSplitOnWord: (value: boolean) => void;
  decodeSuppressNst: boolean;
  setDecodeSuppressNst: (value: boolean) => void;
  decodeTdrzEnable: boolean;
  setDecodeTdrzEnable: (value: boolean) => void;
};

export function DecodeOptions({
  showDecodeOptions,
  setShowDecodeOptions,
  isTauri,
  isBusy,
  decodeOffsetMs,
  setDecodeOffsetMs,
  decodeDurationMs,
  setDecodeDurationMs,
  decodeWordThold,
  setDecodeWordThold,
  decodeMaxLen,
  setDecodeMaxLen,
  decodeMaxTokens,
  setDecodeMaxTokens,
  decodeTemperature,
  setDecodeTemperature,
  decodeTemperatureInc,
  setDecodeTemperatureInc,
  decodeEntropyThold,
  setDecodeEntropyThold,
  decodeLogprobThold,
  setDecodeLogprobThold,
  decodeNoSpeechThold,
  setDecodeNoSpeechThold,
  decodeNoContext,
  setDecodeNoContext,
  decodeNoTimestamps,
  setDecodeNoTimestamps,
  decodeTokenTimestamps,
  setDecodeTokenTimestamps,
  decodeSplitOnWord,
  setDecodeSplitOnWord,
  decodeSuppressNst,
  setDecodeSuppressNst,
  decodeTdrzEnable,
  setDecodeTdrzEnable,
}: DecodeOptionsProps) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[22px] border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/60 p-4 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_70%,transparent)]">
      <div className="flex items-start justify-between gap-5">
        <div className="space-y-1">
          <Label htmlFor="show-decode-options" className="text-base font-semibold text-foreground">
            {t('pages.audio.decode.title')}
          </Label>
          <p className="text-sm leading-5 text-muted-foreground">
            {t('pages.audio.decode.description')}
          </p>
        </div>
        <Switch
          id="show-decode-options"
          checked={showDecodeOptions}
          onCheckedChange={setShowDecodeOptions}
          disabled={!isTauri || isBusy}
        />
      </div>

      {showDecodeOptions && (
        <div className="mt-4 space-y-4 border-t border-border/60 pt-4">
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="decode-offset-ms" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.offset')}
              </Label>
              <Input
                id="decode-offset-ms"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeOffsetMs}
                onChange={(e) => setDecodeOffsetMs(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.offset')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-duration-ms" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.duration')}
              </Label>
              <Input
                id="decode-duration-ms"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeDurationMs}
                onChange={(e) => setDecodeDurationMs(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.duration')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-word-thold" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.wordThreshold')}
              </Label>
              <Input
                id="decode-word-thold"
                type="number"
                inputMode="decimal"
                min={0}
                max={1}
                step={0.01}
                value={decodeWordThold}
                onChange={(e) => setDecodeWordThold(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.wordThreshold')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-max-len" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.maxSegmentLength')}
              </Label>
              <Input
                id="decode-max-len"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeMaxLen}
                onChange={(e) => setDecodeMaxLen(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.maxSegmentLength')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-max-tokens" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.maxTokensPerSegment')}
              </Label>
              <Input
                id="decode-max-tokens"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeMaxTokens}
                onChange={(e) => setDecodeMaxTokens(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.maxTokensPerSegment')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-temperature" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.temperature')}
              </Label>
              <Input
                id="decode-temperature"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.01}
                value={decodeTemperature}
                onChange={(e) => setDecodeTemperature(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.temperature')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-temperature-inc" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.temperatureIncrement')}
              </Label>
              <Input
                id="decode-temperature-inc"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.01}
                value={decodeTemperatureInc}
                onChange={(e) => setDecodeTemperatureInc(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.temperatureIncrement')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-entropy-thold" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.entropyThreshold')}
              </Label>
              <Input
                id="decode-entropy-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeEntropyThold}
                onChange={(e) => setDecodeEntropyThold(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.entropyThreshold')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-logprob-thold" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.logprobThreshold')}
              </Label>
              <Input
                id="decode-logprob-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeLogprobThold}
                onChange={(e) => setDecodeLogprobThold(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.logprobThreshold')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-no-speech-thold" className="text-xs font-semibold text-foreground">
                {t('pages.audio.decode.fields.noSpeechThreshold')}
              </Label>
              <Input
                id="decode-no-speech-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeNoSpeechThold}
                onChange={(e) => setDecodeNoSpeechThold(e.target.value)}
                placeholder={t('pages.audio.decode.placeholders.noSpeechThreshold')}
                disabled={isBusy}
                className="h-11 rounded-xl border-border/70 bg-[var(--shell-card)] shadow-none"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-no-context" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.noContext')}
              </Label>
              <Switch id="decode-no-context" checked={decodeNoContext} onCheckedChange={setDecodeNoContext} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-no-timestamps" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.noTimestamps')}
              </Label>
              <Switch id="decode-no-timestamps" checked={decodeNoTimestamps} onCheckedChange={setDecodeNoTimestamps} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-token-timestamps" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.tokenTimestamps')}
              </Label>
              <Switch id="decode-token-timestamps" checked={decodeTokenTimestamps} onCheckedChange={setDecodeTokenTimestamps} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-split-on-word" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.splitOnWord')}
              </Label>
              <Switch id="decode-split-on-word" checked={decodeSplitOnWord} onCheckedChange={setDecodeSplitOnWord} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-suppress-nst" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.suppressNst')}
              </Label>
              <Switch id="decode-suppress-nst" checked={decodeSuppressNst} onCheckedChange={setDecodeSuppressNst} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-border/70 bg-[var(--shell-card)] px-4 py-3">
              <Label htmlFor="decode-tdrz-enable" className="text-sm font-medium text-foreground">
                {t('pages.audio.decode.fields.enableTinyDiarize')}
              </Label>
              <Switch id="decode-tdrz-enable" checked={decodeTdrzEnable} onCheckedChange={setDecodeTdrzEnable} disabled={isBusy} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
