import type { ChangeEvent, RefObject } from 'react';
import type { NavigateFunction } from 'react-router-dom';
import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Button } from '@slab/components/button';
import { Input } from '@slab/components/input';
import { SoftPanel } from '@slab/components/workspace';
import { FileAudio2, Loader2 } from 'lucide-react';
import type { SelectedFile } from '@/hooks/use-file';
import type { CatalogModel } from '@/lib/api/models';
import type { PreparingStage } from '../const';
import { VadSettings } from './vad-settings';
import { DecodeOptions } from './decode-options';

export type AudioWorkbenchProps = {
  canStartTranscription: boolean;
  catalogModelsError: unknown;
  catalogModelsLoading: boolean;
  decodeEntropyThold: string;
  decodeDurationMs: string;
  decodeLogprobThold: string;
  decodeMaxLen: string;
  decodeMaxTokens: string;
  decodeNoContext: boolean;
  decodeNoSpeechThold: string;
  decodeNoTimestamps: boolean;
  decodeOffsetMs: string;
  decodeSplitOnWord: boolean;
  decodeSuppressNst: boolean;
  decodeTdrzEnable: boolean;
  decodeTemperature: string;
  decodeTemperatureInc: string;
  decodeTokenTimestamps: boolean;
  decodeWordThold: string;
  enableVad: boolean;
  file: SelectedFile | null;
  handleFileChange: (e: ChangeEvent<HTMLInputElement>) => void | Promise<void>;
  handleTauriFileSelect: () => void | Promise<void>;
  handleTranscribe: () => void | Promise<void>;
  isBusy: boolean;
  isTauri: boolean;
  navigate: NavigateFunction;
  preparingStage: PreparingStage;
  previewRows: Array<{ label: string; value: string; accent: boolean; chip: boolean }>;
  selectedVadModel: CatalogModel | undefined;
  selectedVadModelId: string;
  setDecodeEntropyThold: (value: string) => void;
  setDecodeDurationMs: (value: string) => void;
  setDecodeLogprobThold: (value: string) => void;
  setDecodeMaxLen: (value: string) => void;
  setDecodeMaxTokens: (value: string) => void;
  setDecodeNoContext: (value: boolean) => void;
  setDecodeNoSpeechThold: (value: string) => void;
  setDecodeNoTimestamps: (value: boolean) => void;
  setDecodeOffsetMs: (value: string) => void;
  setDecodeSplitOnWord: (value: boolean) => void;
  setDecodeSuppressNst: (value: boolean) => void;
  setDecodeTdrzEnable: (value: boolean) => void;
  setDecodeTemperature: (value: string) => void;
  setDecodeTemperatureInc: (value: string) => void;
  setDecodeTokenTimestamps: (value: boolean) => void;
  setDecodeWordThold: (value: string) => void;
  setEnableVad: (value: boolean) => void;
  setSelectedVadModelId: (value: string) => void;
  setShowDecodeOptions: (value: boolean) => void;
  setVadMaxSpeechDurationS: (value: string) => void;
  setVadMinSilenceDurationMs: (value: string) => void;
  setVadMinSpeechDurationMs: (value: string) => void;
  setVadSamplesOverlap: (value: string) => void;
  setVadSpeechPadMs: (value: string) => void;
  setVadThreshold: (value: string) => void;
  showDecodeOptions: boolean;
  taskId: string | null;
  transcribe: { isError: boolean; error: unknown; isPending: boolean };
  vadMaxSpeechDurationS: string;
  vadMinSilenceDurationMs: string;
  vadMinSpeechDurationMs: string;
  vadSamplesOverlap: string;
  vadSpeechPadMs: string;
  vadThreshold: string;
  webFileInputRef: RefObject<HTMLInputElement | null>;
  whisperVadModels: CatalogModel[];
};

export function AudioWorkbench({
  canStartTranscription,
  catalogModelsError,
  catalogModelsLoading,
  decodeEntropyThold,
  decodeDurationMs,
  decodeLogprobThold,
  decodeMaxLen,
  decodeMaxTokens,
  decodeNoContext,
  decodeNoSpeechThold,
  decodeNoTimestamps,
  decodeOffsetMs,
  decodeSplitOnWord,
  decodeSuppressNst,
  decodeTdrzEnable,
  decodeTemperature,
  decodeTemperatureInc,
  decodeTokenTimestamps,
  decodeWordThold,
  enableVad,
  file,
  handleFileChange,
  handleTauriFileSelect,
  handleTranscribe,
  isBusy,
  isTauri,
  navigate,
  preparingStage,
  previewRows,
  selectedVadModel,
  selectedVadModelId,
  setDecodeEntropyThold,
  setDecodeDurationMs,
  setDecodeLogprobThold,
  setDecodeMaxLen,
  setDecodeMaxTokens,
  setDecodeNoContext,
  setDecodeNoSpeechThold,
  setDecodeNoTimestamps,
  setDecodeOffsetMs,
  setDecodeSplitOnWord,
  setDecodeSuppressNst,
  setDecodeTdrzEnable,
  setDecodeTemperature,
  setDecodeTemperatureInc,
  setDecodeTokenTimestamps,
  setDecodeWordThold,
  setEnableVad,
  setSelectedVadModelId,
  setShowDecodeOptions,
  setVadMaxSpeechDurationS,
  setVadMinSilenceDurationMs,
  setVadMinSpeechDurationMs,
  setVadSamplesOverlap,
  setVadSpeechPadMs,
  setVadThreshold,
  showDecodeOptions,
  taskId,
  transcribe,
  vadMaxSpeechDurationS,
  vadMinSilenceDurationMs,
  vadMinSpeechDurationMs,
  vadSamplesOverlap,
  vadSpeechPadMs,
  vadThreshold,
  webFileInputRef,
  whisperVadModels,
}: AudioWorkbenchProps) {
  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto grid w-full max-w-[1120px] gap-8 pb-8 xl:grid-cols-[minmax(0,1fr)_392px]">
        <div className="space-y-6">
          <SoftPanel className="space-y-5 rounded-[28px] border border-border/60 bg-[var(--surface-soft)] px-7 py-6">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                Transcription Setup
              </p>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                The active Whisper model comes from the global header.
              </p>
            </div>

            {Boolean(catalogModelsError) && (
              <Alert variant="destructive">
                <AlertTitle>Model Catalog Error</AlertTitle>
                <AlertDescription>
                  {(catalogModelsError as { message?: string })?.message ||
                    'Failed to load model catalog. Please check server status.'}
                </AlertDescription>
              </Alert>
            )}

            {transcribe?.isError && (
              <Alert variant="destructive">
                <AlertTitle>Error</AlertTitle>
                <AlertDescription>
                  {(transcribe?.error as { error?: string })?.error ||
                    'Failed to create transcription task. Please retry.'}
                </AlertDescription>
              </Alert>
            )}

            <VadSettings
              enableVad={enableVad}
              setEnableVad={setEnableVad}
              isTauri={isTauri}
              isBusy={isBusy}
              selectedVadModelId={selectedVadModelId}
              setSelectedVadModelId={setSelectedVadModelId}
              catalogModelsLoading={catalogModelsLoading}
              whisperVadModels={whisperVadModels}
              selectedVadModel={selectedVadModel}
              vadThreshold={vadThreshold}
              setVadThreshold={setVadThreshold}
              vadMinSpeechDurationMs={vadMinSpeechDurationMs}
              setVadMinSpeechDurationMs={setVadMinSpeechDurationMs}
              vadMinSilenceDurationMs={vadMinSilenceDurationMs}
              setVadMinSilenceDurationMs={setVadMinSilenceDurationMs}
              vadMaxSpeechDurationS={vadMaxSpeechDurationS}
              setVadMaxSpeechDurationS={setVadMaxSpeechDurationS}
              vadSpeechPadMs={vadSpeechPadMs}
              setVadSpeechPadMs={setVadSpeechPadMs}
              vadSamplesOverlap={vadSamplesOverlap}
              setVadSamplesOverlap={setVadSamplesOverlap}
            />

            <DecodeOptions
              showDecodeOptions={showDecodeOptions}
              setShowDecodeOptions={setShowDecodeOptions}
              isTauri={isTauri}
              isBusy={isBusy}
              decodeOffsetMs={decodeOffsetMs}
              setDecodeOffsetMs={setDecodeOffsetMs}
              decodeDurationMs={decodeDurationMs}
              setDecodeDurationMs={setDecodeDurationMs}
              decodeWordThold={decodeWordThold}
              setDecodeWordThold={setDecodeWordThold}
              decodeMaxLen={decodeMaxLen}
              setDecodeMaxLen={setDecodeMaxLen}
              decodeMaxTokens={decodeMaxTokens}
              setDecodeMaxTokens={setDecodeMaxTokens}
              decodeTemperature={decodeTemperature}
              setDecodeTemperature={setDecodeTemperature}
              decodeTemperatureInc={decodeTemperatureInc}
              setDecodeTemperatureInc={setDecodeTemperatureInc}
              decodeEntropyThold={decodeEntropyThold}
              setDecodeEntropyThold={setDecodeEntropyThold}
              decodeLogprobThold={decodeLogprobThold}
              setDecodeLogprobThold={setDecodeLogprobThold}
              decodeNoSpeechThold={decodeNoSpeechThold}
              setDecodeNoSpeechThold={setDecodeNoSpeechThold}
              decodeNoContext={decodeNoContext}
              setDecodeNoContext={setDecodeNoContext}
              decodeNoTimestamps={decodeNoTimestamps}
              setDecodeNoTimestamps={setDecodeNoTimestamps}
              decodeTokenTimestamps={decodeTokenTimestamps}
              setDecodeTokenTimestamps={setDecodeTokenTimestamps}
              decodeSplitOnWord={decodeSplitOnWord}
              setDecodeSplitOnWord={setDecodeSplitOnWord}
              decodeSuppressNst={decodeSuppressNst}
              setDecodeSuppressNst={setDecodeSuppressNst}
              decodeTdrzEnable={decodeTdrzEnable}
              setDecodeTdrzEnable={setDecodeTdrzEnable}
            />
          </SoftPanel>
          <SoftPanel className="space-y-5 rounded-[28px] border border-border/60 bg-[var(--surface-soft)] px-7 py-6">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                Source Audio
              </p>
            </div>

            <div className="rounded-[24px] border border-dashed border-border/50 bg-[var(--shell-card)]/40 px-6 py-8 text-center">
              <div className="mx-auto flex size-14 items-center justify-center rounded-full bg-[var(--shell-card)]/85 text-[var(--brand-teal)] shadow-[0_18px_34px_-24px_color-mix(in_oklab,var(--brand-teal)_38%,transparent)]">
                <FileAudio2 className="size-6" />
              </div>
              <h3 className="mt-5 text-[18px] font-semibold tracking-[-0.02em] text-foreground">
                Drag and drop audio files
              </h3>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                Supports FLAC, WAV, MP3, M4A, OGG, and common video containers.
              </p>
              <div className="mt-5 flex flex-wrap items-center justify-center gap-3">
                <Button
                  type="button"
                  variant="pill"
                  size="pill"
                  className="rounded-[14px] bg-[var(--shell-card)]"
                  onClick={() => {
                    if (isTauri) {
                      void handleTauriFileSelect();
                      return;
                    }

                    webFileInputRef.current?.click();
                  }}
                  disabled={isBusy}
                >
                  {file ? 'Change File' : 'Browse Files'}
                </Button>
                {file ? (
                  <span className="max-w-full rounded-full border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/80 px-4 py-2 text-xs font-medium text-muted-foreground">
                    {file.name}
                  </span>
                ) : null}
              </div>
              {!isTauri ? (
                <Input
                  ref={webFileInputRef}
                  id="file"
                  type="file"
                  accept="audio/*,video/*"
                  onChange={handleFileChange}
                  disabled={isBusy}
                  className="hidden"
                />
              ) : null}
            </div>

            {file ? (
              <div className="rounded-[20px] border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/60 px-4 py-4 shadow-[inset_0_1px_0_color-mix(in_oklab,var(--shell-card)_70%,transparent)]">
                <p className="text-[12px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
                  Selected File
                </p>
                <p className="mt-2 truncate text-sm font-semibold text-foreground">{file.name}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  Ready for transcription. You can swap the file at any time before creating the
                  task.
                </p>
              </div>
            ) : null}
          </SoftPanel>
        </div>
        <div className="workspace-surface h-fit rounded-[30px] px-7 py-8 shadow-[0_28px_72px_-44px_color-mix(in_oklab,var(--foreground)_34%,transparent)]">
          <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
            Configuration Preview
          </p>

          <div className="mt-7 space-y-4">
            {previewRows.map((item) => (
              <div
                key={item.label}
                className="flex items-start justify-between gap-4 border-b border-border/60 pb-4 last:border-b-0 last:pb-0"
              >
                <p className="pt-1 text-sm text-muted-foreground">{item.label}</p>
                {item.chip ? (
                  <span className="max-w-[220px] rounded-md bg-[color:color-mix(in_oklab,var(--brand-teal)_10%,var(--background))] px-2.5 py-1 text-right text-xs font-semibold text-[var(--brand-teal)]">
                    {item.value}
                  </span>
                ) : (
                  <p
                    className={`max-w-[220px] text-right text-sm font-semibold leading-6 ${
                      item.accent ? 'text-[var(--brand-teal)]' : 'text-foreground'
                    }`}
                  >
                    {item.value}
                  </p>
                )}
              </div>
            ))}
          </div>

          <Button
            variant="cta"
            size="pill"
            className="mt-8 h-14 w-full rounded-[14px] text-base font-semibold"
            onClick={handleTranscribe}
            disabled={!canStartTranscription}
          >
            {isBusy ? <Loader2 className="size-4 animate-spin" /> : null}
            {preparingStage === 'prepare'
              ? 'Preparing Model...'
              : preparingStage === 'transcribe' || transcribe?.isPending
                ? 'Processing...'
                : 'Start Transcription'}
          </Button>

          <p className="mx-auto mt-4 max-w-[290px] text-center text-xs leading-5 text-muted-foreground">
            By starting, the selected file is sent through the current transcription flow and can
            be tracked in Tasks.
          </p>

          {isBusy || taskId || file ? (
            <div className="mt-6 rounded-[22px] bg-[var(--surface-soft)] p-4">
              {isBusy ? (
                <div className="flex items-start gap-3">
                  <div className="mt-0.5 flex size-10 items-center justify-center rounded-full bg-[var(--shell-card)] text-[var(--brand-teal)]">
                    <Loader2 className="size-5 animate-spin" />
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm font-semibold text-foreground">
                      {preparingStage === 'prepare'
                        ? 'Preparing selected model'
                        : 'Creating transcription task'}
                    </p>
                    <p className="text-xs leading-5 text-muted-foreground">
                      {preparingStage === 'prepare'
                        ? 'The runtime is making sure the required model is downloaded and loaded first.'
                        : 'The transcription request is being submitted to the existing task pipeline.'}
                    </p>
                    {taskId ? (
                      <p className="text-xs font-medium text-[var(--brand-teal)]">Task ID: {taskId}</p>
                    ) : null}
                  </div>
                </div>
              ) : taskId ? (
                <div className="space-y-4">
                  <div className="space-y-1">
                    <p className="text-sm font-semibold text-foreground">Transcription task created</p>
                    <p className="text-xs leading-5 text-muted-foreground">
                      Task ID: {taskId}. You can keep working here or jump straight to the Tasks page
                      to monitor progress.
                    </p>
                  </div>
                  <Button variant="pill" size="pill" onClick={() => navigate('/task')}>
                    Open Tasks
                  </Button>
                </div>
              ) : (
                <div className="space-y-1">
                  <p className="text-sm font-semibold text-foreground">Source file ready</p>
                  <p className="truncate text-sm text-muted-foreground">{file?.name ?? 'Source file selected'}</p>
                  <p className="text-xs leading-5 text-muted-foreground">
                    Start Transcription will create a task without changing the existing backend flow.
                  </p>
                </div>
              )}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
