import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';

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
  return (
    <div className="rounded-[22px] border border-white/70 bg-white/60 p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.7)]">
      <div className="flex items-start justify-between gap-5">
        <div className="space-y-1">
          <Label htmlFor="show-decode-options" className="text-base font-semibold text-[#191c1e]">
            Advanced Decode Options
          </Label>
          <p className="text-sm leading-5 text-muted-foreground">Expose manual whisper.cpp knobs only when you need custom behavior.</p>
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
              <Label htmlFor="decode-offset-ms" className="text-xs font-semibold text-[#191c1e]">
                Offset (ms)
              </Label>
              <Input
                id="decode-offset-ms"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeOffsetMs}
                onChange={(e) => setDecodeOffsetMs(e.target.value)}
                placeholder="0"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-duration-ms" className="text-xs font-semibold text-[#191c1e]">
                Duration (ms)
              </Label>
              <Input
                id="decode-duration-ms"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeDurationMs}
                onChange={(e) => setDecodeDurationMs(e.target.value)}
                placeholder="0 (full)"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-word-thold" className="text-xs font-semibold text-[#191c1e]">
                Word Threshold
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
                placeholder="0.01"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-max-len" className="text-xs font-semibold text-[#191c1e]">
                Max Segment Length
              </Label>
              <Input
                id="decode-max-len"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeMaxLen}
                onChange={(e) => setDecodeMaxLen(e.target.value)}
                placeholder="0 (no limit)"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-max-tokens" className="text-xs font-semibold text-[#191c1e]">
                Max Tokens / Segment
              </Label>
              <Input
                id="decode-max-tokens"
                type="number"
                inputMode="numeric"
                min={0}
                step={1}
                value={decodeMaxTokens}
                onChange={(e) => setDecodeMaxTokens(e.target.value)}
                placeholder="0 (no limit)"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-temperature" className="text-xs font-semibold text-[#191c1e]">
                Temperature
              </Label>
              <Input
                id="decode-temperature"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.01}
                value={decodeTemperature}
                onChange={(e) => setDecodeTemperature(e.target.value)}
                placeholder="0.00"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-temperature-inc" className="text-xs font-semibold text-[#191c1e]">
                Temperature Increment
              </Label>
              <Input
                id="decode-temperature-inc"
                type="number"
                inputMode="decimal"
                min={0}
                step={0.01}
                value={decodeTemperatureInc}
                onChange={(e) => setDecodeTemperatureInc(e.target.value)}
                placeholder="0.20"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-entropy-thold" className="text-xs font-semibold text-[#191c1e]">
                Entropy Threshold
              </Label>
              <Input
                id="decode-entropy-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeEntropyThold}
                onChange={(e) => setDecodeEntropyThold(e.target.value)}
                placeholder="2.40"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-logprob-thold" className="text-xs font-semibold text-[#191c1e]">
                Logprob Threshold
              </Label>
              <Input
                id="decode-logprob-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeLogprobThold}
                onChange={(e) => setDecodeLogprobThold(e.target.value)}
                placeholder="-1.00"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="decode-no-speech-thold" className="text-xs font-semibold text-[#191c1e]">
                No Speech Threshold
              </Label>
              <Input
                id="decode-no-speech-thold"
                type="number"
                inputMode="decimal"
                step={0.01}
                value={decodeNoSpeechThold}
                onChange={(e) => setDecodeNoSpeechThold(e.target.value)}
                placeholder="0.60"
                disabled={isBusy}
                className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
              />
            </div>
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-no-context" className="text-sm font-medium text-[#191c1e]">
                No Context
              </Label>
              <Switch id="decode-no-context" checked={decodeNoContext} onCheckedChange={setDecodeNoContext} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-no-timestamps" className="text-sm font-medium text-[#191c1e]">
                No Timestamps
              </Label>
              <Switch id="decode-no-timestamps" checked={decodeNoTimestamps} onCheckedChange={setDecodeNoTimestamps} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-token-timestamps" className="text-sm font-medium text-[#191c1e]">
                Token Timestamps
              </Label>
              <Switch id="decode-token-timestamps" checked={decodeTokenTimestamps} onCheckedChange={setDecodeTokenTimestamps} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-split-on-word" className="text-sm font-medium text-[#191c1e]">
                Split On Word
              </Label>
              <Switch id="decode-split-on-word" checked={decodeSplitOnWord} onCheckedChange={setDecodeSplitOnWord} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-suppress-nst" className="text-sm font-medium text-[#191c1e]">
                Suppress NST
              </Label>
              <Switch id="decode-suppress-nst" checked={decodeSuppressNst} onCheckedChange={setDecodeSuppressNst} disabled={isBusy} />
            </div>
            <div className="flex items-center justify-between rounded-[16px] border border-[#dbe4ea] bg-white px-4 py-3">
              <Label htmlFor="decode-tdrz-enable" className="text-sm font-medium text-[#191c1e]">
                Enable TinyDiarize
              </Label>
              <Switch id="decode-tdrz-enable" checked={decodeTdrzEnable} onCheckedChange={setDecodeTdrzEnable} disabled={isBusy} />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
