import type { ChangeEvent, RefObject } from 'react';
import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Button } from '@slab/components/button';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '@slab/components/dialog';
import { Input } from '@slab/components/input';
import { SoftPanel } from '@slab/components/workspace';
import { useTranslation } from '@slab/i18n';
import { FileAudio2, History, Loader2 } from 'lucide-react';
import type { SelectedFile } from '@/hooks/use-file';
import type { CatalogModel } from '@/lib/api/models';
import type { AudioTranscriptionTask } from '@/lib/media-task-api';
import type { PreparingStage } from '../const';
import { VadSettings } from './vad-settings';
import { DecodeOptions } from './decode-options';

export type AudioWorkbenchProps = {
  bundledVadLabel: string;
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
  hasBundledVad: boolean;
  history: AudioTranscriptionTask[];
  historyDialogOpen: boolean;
  historyError: string | null;
  historyLoading: boolean;
  isBusy: boolean;
  isTauri: boolean;
  isUsingBundledVad: boolean;
  openHistoryDetail: (taskId: string) => void | Promise<void>;
  preparingStage: PreparingStage;
  previewRows: Array<{ label: string; value: string; accent: boolean; chip: boolean }>;
  selectedHistoryTask: AudioTranscriptionTask | null;
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
  setHistoryDialogOpen: (open: boolean) => void;
  setSelectedVadModelId: (value: string) => void;
  setSelectedHistoryTask: (task: AudioTranscriptionTask | null) => void;
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

function formatHistoryTime(value: string) {
  return new Date(value).toLocaleString(undefined, {
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function AudioWorkbench({
  bundledVadLabel,
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
  hasBundledVad,
  history,
  historyDialogOpen,
  historyError,
  historyLoading,
  isBusy,
  isTauri,
  isUsingBundledVad,
  openHistoryDetail,
  preparingStage,
  previewRows,
  selectedHistoryTask,
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
  setHistoryDialogOpen,
  setSelectedVadModelId,
  setSelectedHistoryTask,
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
  const { t } = useTranslation();
  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto grid w-full max-w-[1120px] gap-8 pb-8 lg:grid-cols-[minmax(0,1fr)_360px] xl:grid-cols-[minmax(0,1fr)_392px]">
        <div className="space-y-6">
          <SoftPanel className="space-y-5 rounded-[28px] border border-border/60 bg-[var(--surface-soft)] px-7 py-6">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
                {t('pages.audio.workbench.setupTitle')}
              </p>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                {t('pages.audio.workbench.setupDescription')}
              </p>
            </div>

            {Boolean(catalogModelsError) && (
              <Alert variant="destructive">
                <AlertTitle>{t('pages.audio.alerts.catalogErrorTitle')}</AlertTitle>
                <AlertDescription>
                  {(catalogModelsError as { message?: string })?.message ||
                    t('pages.audio.alerts.catalogErrorDescription')}
                </AlertDescription>
              </Alert>
            )}

            {transcribe?.isError && (
              <Alert variant="destructive">
                <AlertTitle>{t('pages.audio.alerts.transcribeErrorTitle')}</AlertTitle>
                <AlertDescription>
                  {(transcribe?.error as { error?: string })?.error ||
                    t('pages.audio.alerts.transcribeErrorDescription')}
                </AlertDescription>
              </Alert>
            )}

            <VadSettings
              bundledVadLabel={bundledVadLabel}
              enableVad={enableVad}
              hasBundledVad={hasBundledVad}
              setEnableVad={setEnableVad}
              isTauri={isTauri}
              isBusy={isBusy}
              isUsingBundledVad={isUsingBundledVad}
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
                {t('pages.audio.workbench.sourceTitle')}
              </p>
            </div>

            <div className="rounded-[24px] border border-dashed border-border/50 bg-[var(--shell-card)]/40 px-6 py-8 text-center">
              <div className="mx-auto flex size-14 items-center justify-center rounded-full bg-[var(--shell-card)]/85 text-[var(--brand-teal)] shadow-[0_18px_34px_-24px_color-mix(in_oklab,var(--brand-teal)_38%,transparent)]">
                <FileAudio2 className="size-6" />
              </div>
              <h3 className="mt-5 text-[18px] font-semibold tracking-[-0.02em] text-foreground">
                {t('pages.audio.workbench.sourceDropTitle')}
              </h3>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                {t('pages.audio.workbench.sourceDropDescription')}
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
                  {file
                    ? t('pages.audio.workbench.changeFile')
                    : t('pages.audio.workbench.browseFiles')}
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
                  {t('pages.audio.workbench.selectedFileTitle')}
                </p>
                <p className="mt-2 truncate text-sm font-semibold text-foreground">{file.name}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  {t('pages.audio.workbench.selectedFileDescription')}
                </p>
              </div>
            ) : null}
          </SoftPanel>
        </div>
        <div className="workspace-surface h-fit rounded-[30px] px-7 py-8 shadow-[0_28px_72px_-44px_color-mix(in_oklab,var(--foreground)_34%,transparent)]">
          <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-muted-foreground">
            {t('pages.audio.workbench.previewTitle')}
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
              ? t('pages.audio.workbench.startPreparing')
              : preparingStage === 'transcribe' || transcribe?.isPending
                ? t('pages.audio.workbench.startProcessing')
                : t('pages.audio.workbench.startTranscription')}
          </Button>

          <p className="mx-auto mt-4 max-w-[290px] text-center text-xs leading-5 text-muted-foreground">
            {t('pages.audio.workbench.submitDescription')}
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
                        ? t('pages.audio.workbench.busy.preparingTitle')
                        : t('pages.audio.workbench.busy.creatingTitle')}
                    </p>
                    <p className="text-xs leading-5 text-muted-foreground">
                      {preparingStage === 'prepare'
                        ? t('pages.audio.workbench.busy.preparingDescription')
                        : t('pages.audio.workbench.busy.creatingDescription')}
                    </p>
                    {taskId ? (
                      <p className="text-xs font-medium text-[var(--brand-teal)]">
                        {t('pages.audio.workbench.taskIdLabel', { id: taskId })}
                      </p>
                    ) : null}
                  </div>
                </div>
              ) : taskId ? (
                <div className="space-y-4">
                  <div className="space-y-1">
                    <p className="text-sm font-semibold text-foreground">
                      {t('pages.audio.workbench.taskCreated.title')}
                    </p>
                    <p className="text-xs leading-5 text-muted-foreground">
                      {t('pages.audio.workbench.taskCreated.description', { id: taskId })}
                    </p>
                  </div>
                  <Button
                    variant="pill"
                    size="pill"
                    onClick={() => {
                      if (selectedHistoryTask) {
                        setHistoryDialogOpen(true);
                        return;
                      }

                      void openHistoryDetail(taskId);
                    }}
                  >
                    {t('pages.audio.workbench.taskCreated.viewTranscript')}
                  </Button>
                </div>
              ) : (
                <div className="space-y-1">
                  <p className="text-sm font-semibold text-foreground">
                    {t('pages.audio.workbench.ready.title')}
                  </p>
                  <p className="truncate text-sm text-muted-foreground">
                    {file?.name ?? t('pages.audio.workbench.ready.fallbackSource')}
                  </p>
                  <p className="text-xs leading-5 text-muted-foreground">
                    {t('pages.audio.workbench.ready.description')}
                  </p>
                </div>
              )}
            </div>
          ) : null}

          <div className="mt-6 rounded-[22px] bg-[var(--surface-soft)] p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-[11px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">
                  {t('pages.audio.history.title')}
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {historyLoading
                    ? t('pages.audio.history.loading')
                    : historyError
                      ? t('pages.audio.history.error', { message: historyError })
                      : t('pages.audio.history.description')}
                </p>
              </div>
              <History className="size-4 text-muted-foreground" />
            </div>
            <div className="mt-3 space-y-3">
              {history.slice(0, 4).map((task) => (
                <button
                  key={task.task_id}
                  type="button"
                  className="w-full rounded-[18px] border border-border/50 bg-[var(--shell-card)] px-4 py-3 text-left transition hover:border-[var(--brand-teal)]/50"
                  onClick={() => void openHistoryDetail(task.task_id)}
                >
                  <p className="line-clamp-2 text-sm font-semibold text-foreground">
                    {task.transcript_text?.slice(0, 96) ||
                      task.prompt ||
                      task.source_path}
                  </p>
                  <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                    <span className="rounded-full bg-[var(--surface-soft)] px-2 py-0.5">
                      {task.status}
                    </span>
                    <span>{formatHistoryTime(task.created_at)}</span>
                  </div>
                </button>
              ))}
              {!historyLoading && history.length === 0 ? (
                <p className="rounded-[18px] border border-dashed border-border/60 bg-[var(--shell-card)] px-4 py-5 text-sm text-muted-foreground">
                  {t('pages.audio.history.empty')}
                </p>
              ) : null}
            </div>
          </div>
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
                <DialogTitle>{t('pages.audio.history.detailTitle')}</DialogTitle>
                <DialogDescription>
                  {selectedHistoryTask.status} | {formatHistoryTime(selectedHistoryTask.created_at)}
                </DialogDescription>
              </DialogHeader>
              <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
                <div className="max-h-[68vh] overflow-y-auto rounded-[22px] bg-[var(--surface-soft)] p-4">
                  <pre className="whitespace-pre-wrap break-words text-sm leading-6 text-foreground">
                    {selectedHistoryTask.transcript_text ?? t('pages.audio.history.pendingTranscript')}
                  </pre>
                </div>
                <div className="space-y-4 rounded-[22px] bg-[var(--surface-soft)] p-4">
                  <div>
                    <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted-foreground">
                      {t('pages.audio.history.fields.source')}
                    </p>
                    <p className="mt-2 break-all text-sm text-foreground">
                      {selectedHistoryTask.source_path}
                    </p>
                  </div>
                  <div className="grid grid-cols-1 gap-3 text-sm">
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.audio.history.fields.model')}</p>
                      <p className="font-semibold">{selectedHistoryTask.model_id ?? selectedHistoryTask.backend_id}</p>
                    </div>
                    <div>
                      <p className="text-xs text-muted-foreground">{t('pages.audio.history.fields.language')}</p>
                      <p className="font-semibold">{selectedHistoryTask.language ?? '-'}</p>
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
