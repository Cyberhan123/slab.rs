import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { SoftPanel } from '@/components/ui/workspace';
import { FileAudio2, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import useFile, { SelectedFile } from '@/hooks/use-file';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './hooks/use-transcribe';
import useIsTauri from '@/hooks/use-tauri';
import api from '@/lib/api';
import { inferWhisperVadModel, toCatalogModelList } from '@/lib/api/models';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

const WHISPER_BACKEND_ID = 'ggml.whisper';
const MODEL_DOWNLOAD_POLL_INTERVAL_MS = 2_000;
const MODEL_DOWNLOAD_TIMEOUT_MS = 30 * 60 * 1_000;

type PreparingStage = 'prepare' | 'transcribe' | null;

export default function Audio() {
  const navigate = useNavigate();
  const isTauri = useIsTauri();
  usePageHeader(PAGE_HEADER_META.audio);

  // file object or string path (desktop uses string path)
  const [file, setFile] = useState<SelectedFile | null>(null);
  const [selectedModelId, setSelectedModelId] = useState('');
  const [enableVad, setEnableVad] = useState(false);
  const [selectedVadModelId, setSelectedVadModelId] = useState('');
  const [vadThreshold, setVadThreshold] = useState('');
  const [vadMinSpeechDurationMs, setVadMinSpeechDurationMs] = useState('');
  const [vadMinSilenceDurationMs, setVadMinSilenceDurationMs] = useState('');
  const [vadMaxSpeechDurationS, setVadMaxSpeechDurationS] = useState('');
  const [vadSpeechPadMs, setVadSpeechPadMs] = useState('');
  const [vadSamplesOverlap, setVadSamplesOverlap] = useState('');
  const [showDecodeOptions, setShowDecodeOptions] = useState(false);
  const [decodeOffsetMs, setDecodeOffsetMs] = useState('');
  const [decodeDurationMs, setDecodeDurationMs] = useState('');
  const [decodeWordThold, setDecodeWordThold] = useState('');
  const [decodeMaxLen, setDecodeMaxLen] = useState('');
  const [decodeMaxTokens, setDecodeMaxTokens] = useState('');
  const [decodeTemperature, setDecodeTemperature] = useState('');
  const [decodeTemperatureInc, setDecodeTemperatureInc] = useState('');
  const [decodeEntropyThold, setDecodeEntropyThold] = useState('');
  const [decodeLogprobThold, setDecodeLogprobThold] = useState('');
  const [decodeNoSpeechThold, setDecodeNoSpeechThold] = useState('');
  const [decodeNoContext, setDecodeNoContext] = useState(false);
  const [decodeNoTimestamps, setDecodeNoTimestamps] = useState(false);
  const [decodeTokenTimestamps, setDecodeTokenTimestamps] = useState(false);
  const [decodeSplitOnWord, setDecodeSplitOnWord] = useState(false);
  const [decodeSuppressNst, setDecodeSuppressNst] = useState(false);
  const [decodeTdrzEnable, setDecodeTdrzEnable] = useState(false);
  const [preparingStage, setPreparingStage] = useState<PreparingStage>(null);
  const [taskId, setTaskId] = useState<string | null>(null);

  const { handleFile } = useFile();
  const transcribe = useTranscribe();
  const {
    data: catalogModels,
    isLoading: catalogModelsLoading,
    error: catalogModelsError,
    refetch: refetchCatalogModels,
  } = api.useQuery('get', '/v1/models');
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const normalizedCatalogModels = useMemo(
    () => toCatalogModelList(catalogModels),
    [catalogModels],
  );

  const whisperModels = useMemo(
    () => normalizedCatalogModels.filter((model) => model.backend_id === WHISPER_BACKEND_ID),
    [normalizedCatalogModels],
  );

  const whisperTranscribeModels = useMemo(
    () => whisperModels.filter((model) => !inferWhisperVadModel(model)),
    [whisperModels],
  );

  const whisperVadModels = useMemo(
    () => whisperModels.filter((model) => inferWhisperVadModel(model)),
    [whisperModels],
  );

  const selectedModel = useMemo(
    () => whisperTranscribeModels.find((model) => model.id === selectedModelId),
    [whisperTranscribeModels, selectedModelId]
  );

  const selectedVadModel = useMemo(
    () => whisperVadModels.find((model) => model.id === selectedVadModelId),
    [whisperVadModels, selectedVadModelId]
  );

  const isBusy =
    Boolean(preparingStage) ||
    transcribe.isPending ||
    loadModelMutation.isPending ||
    downloadModelMutation.isPending;
  const webFileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (whisperTranscribeModels.length === 0) {
      setSelectedModelId('');
      return;
    }

    const exists = whisperTranscribeModels.some((model) => model.id === selectedModelId);
    if (!selectedModelId || !exists) {
      setSelectedModelId(whisperTranscribeModels[0].id);
    }
  }, [whisperTranscribeModels, selectedModelId]);

  useEffect(() => {
    if (!enableVad) {
      return;
    }

    if (whisperVadModels.length === 0) {
      setSelectedVadModelId('');
      return;
    }

    const exists = whisperVadModels.some((model) => model.id === selectedVadModelId);
    if (!selectedVadModelId || !exists) {
      setSelectedVadModelId(whisperVadModels[0].id);
    }
  }, [enableVad, whisperVadModels, selectedVadModelId]);

  const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

  const extractTaskId = (payload: unknown): string | null => {
    if (typeof payload !== 'object' || payload === null) return null;
    const taskId =
      (payload as { operation_id?: unknown }).operation_id ??
      (payload as { task_id?: unknown }).task_id;
    if (typeof taskId !== 'string') return null;
    const trimmed = taskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const waitForTaskToFinish = async (taskId: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: {
          path: { id: taskId },
        },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(task.error_msg ?? `Task ${taskId} ended with status: ${task.status}`);
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error('Model download timed out');
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const refreshed = await refetchCatalogModels();
    const models = toCatalogModelList(refreshed.data);
    return models.find((model) => model.id === modelId);
  };

  const parseOptionalInt = (raw: string, fieldLabel: string, min: number): number | undefined => {
    const trimmed = raw.trim();
    if (!trimmed) return undefined;

    const parsed = Number(trimmed);
    if (!Number.isInteger(parsed)) {
      throw new Error(`${fieldLabel} must be an integer.`);
    }
    if (parsed < min) {
      throw new Error(`${fieldLabel} must be >= ${min}.`);
    }
    return parsed;
  };

  const parseOptionalFloat = (
    raw: string,
    fieldLabel: string,
    options: { min?: number; max?: number; exclusiveMin?: number } = {}
  ): number | undefined => {
    const trimmed = raw.trim();
    if (!trimmed) return undefined;

    const parsed = Number(trimmed);
    if (!Number.isFinite(parsed)) {
      throw new Error(`${fieldLabel} must be a valid number.`);
    }
    if (options.min !== undefined && parsed < options.min) {
      throw new Error(`${fieldLabel} must be >= ${options.min}.`);
    }
    if (options.max !== undefined && parsed > options.max) {
      throw new Error(`${fieldLabel} must be <= ${options.max}.`);
    }
    if (options.exclusiveMin !== undefined && parsed <= options.exclusiveMin) {
      throw new Error(`${fieldLabel} must be > ${options.exclusiveMin}.`);
    }
    return parsed;
  };

  const ensureDownloadedModelPath = async (
    modelId: string
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = whisperModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.backend_id !== WHISPER_BACKEND_ID) {
      throw new Error(`Selected model does not support ${WHISPER_BACKEND_ID}`);
    }

    if (model.local_path) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    const downloadResponse = await downloadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    });
    const taskId = extractTaskId(downloadResponse);

    if (!taskId) {
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(taskId);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error('Model download completed, but local_path is empty');
    }

    return { modelPath: refreshedModel.local_path, downloadedNow: true };
  };

  const prepareSelectedModel = async (): Promise<string> => {
    if (!selectedModelId) {
      throw new Error('Please select a whisper model first.');
    }

    const model = whisperTranscribeModels.find((item) => item.id === selectedModelId);
    if (!model) {
      throw new Error('Selected model no longer exists in catalog.');
    }

    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(`Downloaded ${model.display_name}`);
    }

    await loadModelMutation.mutateAsync({
      body: {
        backend_id: WHISPER_BACKEND_ID,
        model_path: modelPath,
      },
    });

    return model.display_name;
  };

  const prepareVadSettings = async (): Promise<{ settings: TranscribeVadSettings; modelName: string }> => {
    if (!selectedVadModelId) {
      throw new Error('Please select a dedicated VAD model.');
    }

    let model = whisperVadModels.find((item) => item.id === selectedVadModelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(selectedVadModelId);
    }
    if (!model) {
      throw new Error('Selected VAD model no longer exists in catalog.');
    }
    if (!inferWhisperVadModel(model)) {
      throw new Error('Selected model is not a dedicated VAD model.');
    }

    const { modelPath, downloadedNow } = await ensureDownloadedModelPath(selectedVadModelId);
    if (downloadedNow) {
      toast.success(`Downloaded VAD model ${model.display_name}`);
    }

    const settings: TranscribeVadSettings = {
      enabled: true,
      model_path: modelPath,
    };

    const threshold = parseOptionalFloat(vadThreshold, 'VAD threshold', { min: 0, max: 1 });
    const minSpeechDurationMs = parseOptionalInt(
      vadMinSpeechDurationMs,
      'VAD min speech duration (ms)',
      0
    );
    const minSilenceDurationMs = parseOptionalInt(
      vadMinSilenceDurationMs,
      'VAD min silence duration (ms)',
      0
    );
    const maxSpeechDurationS = parseOptionalFloat(vadMaxSpeechDurationS, 'VAD max speech duration (s)', {
      exclusiveMin: 0,
    });
    const speechPadMs = parseOptionalInt(vadSpeechPadMs, 'VAD speech pad (ms)', 0);
    const samplesOverlap = parseOptionalFloat(vadSamplesOverlap, 'VAD samples overlap (s)', { min: 0 });

    if (threshold !== undefined) settings.threshold = threshold;
    if (minSpeechDurationMs !== undefined) settings.min_speech_duration_ms = minSpeechDurationMs;
    if (minSilenceDurationMs !== undefined) settings.min_silence_duration_ms = minSilenceDurationMs;
    if (maxSpeechDurationS !== undefined) settings.max_speech_duration_s = maxSpeechDurationS;
    if (speechPadMs !== undefined) settings.speech_pad_ms = speechPadMs;
    if (samplesOverlap !== undefined) settings.samples_overlap = samplesOverlap;

    return { settings, modelName: model.display_name };
  };

  const prepareDecodeOptions = (): TranscribeOptions['decode'] | undefined => {
    if (!showDecodeOptions) {
      return undefined;
    }

    const decode: NonNullable<TranscribeOptions['decode']> = {};

    const offsetMs = parseOptionalInt(decodeOffsetMs, 'Decode offset (ms)', 0);
    const durationMs = parseOptionalInt(decodeDurationMs, 'Decode duration (ms)', 0);
    const wordThold = parseOptionalFloat(decodeWordThold, 'Word threshold', { min: 0, max: 1 });
    const maxLen = parseOptionalInt(decodeMaxLen, 'Max segment length', 0);
    const maxTokens = parseOptionalInt(decodeMaxTokens, 'Max tokens per segment', 0);
    const temperature = parseOptionalFloat(decodeTemperature, 'Temperature', { min: 0 });
    const temperatureInc = parseOptionalFloat(decodeTemperatureInc, 'Temperature increment', { min: 0 });
    const entropyThold = parseOptionalFloat(decodeEntropyThold, 'Entropy threshold');
    const logprobThold = parseOptionalFloat(decodeLogprobThold, 'Logprob threshold');
    const noSpeechThold = parseOptionalFloat(decodeNoSpeechThold, 'No speech threshold');

    if (offsetMs !== undefined) decode.offset_ms = offsetMs;
    if (durationMs !== undefined) decode.duration_ms = durationMs;
    if (wordThold !== undefined) decode.word_thold = wordThold;
    if (maxLen !== undefined) decode.max_len = maxLen;
    if (maxTokens !== undefined) decode.max_tokens = maxTokens;
    if (temperature !== undefined) decode.temperature = temperature;
    if (temperatureInc !== undefined) decode.temperature_inc = temperatureInc;
    if (entropyThold !== undefined) decode.entropy_thold = entropyThold;
    if (logprobThold !== undefined) decode.logprob_thold = logprobThold;
    if (noSpeechThold !== undefined) decode.no_speech_thold = noSpeechThold;
    if (decodeNoContext) decode.no_context = true;
    if (decodeNoTimestamps) decode.no_timestamps = true;
    if (decodeTokenTimestamps) decode.token_timestamps = true;
    if (decodeSplitOnWord) decode.split_on_word = true;
    if (decodeSuppressNst) decode.suppress_nst = true;
    if (decodeTdrzEnable) decode.tdrz_enable = true;

    return Object.keys(decode).length > 0 ? decode : undefined;
  };

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    e.preventDefault();
    const selectedFile = await handleFile(e);

    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTauriFileSelect = async () => {
    const selectedFile = await handleFile();
    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTranscribe = async () => {
    if (!isTauri) {
      toast.error('Web transcription upload is not implemented yet. Please use the desktop app.');
      return;
    }

    if (!file) {
      toast.error('Please select a file first.');
      return;
    }

    if (!selectedModelId) {
      toast.error('Please select a whisper model first.');
      return;
    }

    try {
      setTaskId(null);
      setPreparingStage('prepare');
      const modelName = await prepareSelectedModel();
      let vadDescription = 'VAD: off';
      let decodeDescription = 'Decode: default';
      let transcribeOptions: TranscribeOptions | undefined;

      if (enableVad) {
        const preparedVad = await prepareVadSettings();
        transcribeOptions = { vad: preparedVad.settings };
        vadDescription = `VAD: on (${preparedVad.modelName})`;
      }

      const decodeOptions = prepareDecodeOptions();
      if (decodeOptions) {
        transcribeOptions = { ...(transcribeOptions ?? {}), decode: decodeOptions };
        decodeDescription = 'Decode: custom';
      }

      setPreparingStage('transcribe');
      const result = await transcribe.handleTranscribe(file.file, transcribeOptions);
      setTaskId(result.operation_id);

      toast.success('Transcription task created.', {
        description: `Task ID: ${result.operation_id} | Model: ${modelName} | ${vadDescription} | ${decodeDescription}`,
        action: {
          label: 'View tasks',
          onClick: () => navigate('/task')
        }
      });
    } catch (err: any) {
      toast.error('Failed to create transcription task.', {
        description: err?.message || err?.error || 'Unknown error'
      });
    } finally {
      setPreparingStage(null);
    }
  };

  const canStartTranscription =
    isTauri &&
    Boolean(file) &&
    Boolean(selectedModelId) &&
    !isBusy &&
    (!enableVad || Boolean(selectedVadModelId));

  const previewRows = [
    { label: 'Model', value: selectedModel?.display_name ?? 'Not selected', accent: Boolean(selectedModel), chip: true },
    { label: 'Source', value: file?.name ?? 'Awaiting upload', accent: Boolean(file), chip: false },
    {
      label: 'VAD Mode',
      value: enableVad ? (selectedVadModel?.display_name ? `Active (${selectedVadModel.display_name})` : 'Active') : 'Inactive',
      accent: enableVad,
      chip: false,
    },
    { label: 'Decode', value: showDecodeOptions ? 'Custom profile' : 'Default profile', accent: showDecodeOptions, chip: false },
  ];

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto grid w-full max-w-[1120px] gap-8 pb-8 xl:grid-cols-[minmax(0,1fr)_392px]">
        <div className="space-y-6">
          <SoftPanel className="space-y-5 rounded-[28px] border border-[#e2e8ee] bg-[#f2f4f6] px-7 py-6">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-[#6d7a77]">
                Transcription Setup
              </p>
            </div>

            {catalogModelsError && (
              <Alert variant="destructive">
                <AlertTitle>Model Catalog Error</AlertTitle>
                <AlertDescription>
                  {(catalogModelsError as any)?.message ||
                    'Failed to load model catalog. Please check server status.'}
                </AlertDescription>
              </Alert>
            )}

            {transcribe?.isError && (
              <Alert variant="destructive">
                <AlertTitle>Error</AlertTitle>
                <AlertDescription>
                  {(transcribe?.error as any)?.error ||
                    'Failed to create transcription task. Please retry.'}
                </AlertDescription>
              </Alert>
            )}

            <div className="rounded-[22px] border border-white/70 bg-white/60 p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.7)]">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="space-y-1">
                  <Label className="text-[12px] font-semibold text-[#191c1e]">Whisper Model</Label>
                  <p className="text-xs leading-5 text-muted-foreground">
                    Choose the transcription model used for this task.
                  </p>
                </div>
                <Button
                  type="button"
                  variant="quiet"
                  size="sm"
                  className="rounded-full px-3"
                  onClick={() => void refetchCatalogModels()}
                  disabled={isBusy || catalogModelsLoading}
                >
                  Refresh
                </Button>
              </div>

              <Select
                value={selectedModelId}
                onValueChange={setSelectedModelId}
                disabled={!isTauri || isBusy || catalogModelsLoading || whisperTranscribeModels.length === 0}
              >
                <SelectTrigger
                  variant="soft"
                  size="pill"
                  className="mt-4 w-full justify-between border-[#dbe4ea] bg-white shadow-none"
                >
                  <SelectValue
                    placeholder={catalogModelsLoading ? 'Loading models...' : 'Select whisper model'}
                  />
                </SelectTrigger>
                <SelectContent variant="soft">
                  {whisperTranscribeModels.length === 0 ? (
                    <div className="px-2 py-1.5 text-sm text-muted-foreground">
                      No Whisper transcription models in catalog
                    </div>
                  ) : (
                    whisperTranscribeModels.map((model) => (
                      <SelectItem key={model.id} value={model.id}>
                        {model.display_name}
                      </SelectItem>
                    ))
                  )}
                </SelectContent>
              </Select>

              <p className="mt-2 text-[11px] leading-5 text-muted-foreground">
                {selectedModel
                  ? selectedModel.local_path
                    ? 'Downloaded locally and ready to load.'
                    : 'Not downloaded yet. It will be fetched automatically when you start.'
                  : catalogModelsLoading
                    ? 'Loading model catalog...'
                    : 'No Whisper transcription model available. Please add one in Settings first.'}
              </p>
            </div>
            <div className="rounded-[22px] border border-white/70 bg-white/60 p-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.7)]">
              <div className="flex items-start justify-between gap-5">
                <div className="space-y-1">
                  <Label htmlFor="enable-vad" className="text-base font-semibold text-[#191c1e]">
                    Enable VAD
                  </Label>
                  <p className="text-sm leading-5 text-muted-foreground">Trim silence and reduce background noise before decoding.</p>
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
                    <Label className="text-[12px] font-semibold text-[#191c1e]">VAD Model</Label>
                    <Select
                      value={selectedVadModelId}
                      onValueChange={setSelectedVadModelId}
                      disabled={!isTauri || isBusy || whisperVadModels.length === 0}
                    >
                      <SelectTrigger
                        variant="soft"
                        size="pill"
                        className="w-full justify-between border-[#dbe4ea] bg-white shadow-none"
                      >
                        <SelectValue
                          placeholder={catalogModelsLoading ? 'Loading models...' : 'Select VAD model'}
                        />
                      </SelectTrigger>
                      <SelectContent variant="soft">
                        {whisperVadModels.length === 0 ? (
                          <div className="px-2 py-1.5 text-sm text-muted-foreground">
                            No dedicated Whisper VAD models in catalog
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
                      {selectedVadModel
                        ? selectedVadModel.local_path
                          ? 'Downloaded locally and ready for runtime use.'
                          : 'The selected VAD model will be downloaded automatically before transcription.'
                        : 'Choose a dedicated VAD model, such as a whisper VAD or silero variant.'}
                    </p>
                  </div>

                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-threshold" className="text-xs font-semibold text-[#191c1e]">
                        Threshold
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
                        placeholder="0.50"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-min-speech-duration" className="text-xs font-semibold text-[#191c1e]">
                        Min Speech (ms)
                      </Label>
                      <Input
                        id="vad-min-speech-duration"
                        type="number"
                        inputMode="numeric"
                        min={0}
                        step={1}
                        value={vadMinSpeechDurationMs}
                        onChange={(e) => setVadMinSpeechDurationMs(e.target.value)}
                        placeholder="250"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-min-silence-duration" className="text-xs font-semibold text-[#191c1e]">
                        Min Silence (ms)
                      </Label>
                      <Input
                        id="vad-min-silence-duration"
                        type="number"
                        inputMode="numeric"
                        min={0}
                        step={1}
                        value={vadMinSilenceDurationMs}
                        onChange={(e) => setVadMinSilenceDurationMs(e.target.value)}
                        placeholder="100"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-max-speech-duration" className="text-xs font-semibold text-[#191c1e]">
                        Max Speech (s)
                      </Label>
                      <Input
                        id="vad-max-speech-duration"
                        type="number"
                        inputMode="decimal"
                        min={0}
                        step={0.1}
                        value={vadMaxSpeechDurationS}
                        onChange={(e) => setVadMaxSpeechDurationS(e.target.value)}
                        placeholder="No limit"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-speech-pad" className="text-xs font-semibold text-[#191c1e]">
                        Speech Pad (ms)
                      </Label>
                      <Input
                        id="vad-speech-pad"
                        type="number"
                        inputMode="numeric"
                        min={0}
                        step={1}
                        value={vadSpeechPadMs}
                        onChange={(e) => setVadSpeechPadMs(e.target.value)}
                        placeholder="30"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                    <div className="space-y-1.5">
                      <Label htmlFor="vad-samples-overlap" className="text-xs font-semibold text-[#191c1e]">
                        Samples Overlap (s)
                      </Label>
                      <Input
                        id="vad-samples-overlap"
                        type="number"
                        inputMode="decimal"
                        min={0}
                        step={0.01}
                        value={vadSamplesOverlap}
                        onChange={(e) => setVadSamplesOverlap(e.target.value)}
                        placeholder="0.10"
                        disabled={isBusy}
                        className="h-11 rounded-xl border-[#dbe4ea] bg-white shadow-none"
                      />
                    </div>
                  </div>
                </div>
              )}
            </div>
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
          </SoftPanel>
          <SoftPanel className="space-y-5 rounded-[28px] border border-[#e2e8ee] bg-[#f2f4f6] px-7 py-6">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-[#6d7a77]">
                Source Audio
              </p>
            </div>

            <div className="rounded-[24px] border border-dashed border-[#cfd8de] bg-white/40 px-6 py-8 text-center">
              <div className="mx-auto flex size-14 items-center justify-center rounded-full bg-white/85 text-[var(--brand-teal)] shadow-[0_18px_34px_-24px_color-mix(in_oklab,var(--brand-teal)_38%,transparent)]">
                <FileAudio2 className="size-6" />
              </div>
              <h3 className="mt-5 text-[18px] font-semibold tracking-[-0.02em] text-[#191c1e]">
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
                  className="rounded-[14px] bg-white"
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
                  <span className="max-w-full rounded-full border border-white/70 bg-white/80 px-4 py-2 text-xs font-medium text-[#3d4947]">
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
              <div className="rounded-[20px] border border-white/70 bg-white/60 px-4 py-4 shadow-[inset_0_1px_0_rgba(255,255,255,0.7)]">
                <p className="text-[12px] font-semibold uppercase tracking-[0.18em] text-[#6d7a77]">
                  Selected File
                </p>
                <p className="mt-2 truncate text-sm font-semibold text-[#191c1e]">{file.name}</p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  Ready for transcription. You can swap the file at any time before creating the
                  task.
                </p>
              </div>
            ) : null}
          </SoftPanel>
        </div>
        <div className="workspace-surface h-fit rounded-[30px] px-7 py-8 shadow-[0_28px_72px_-44px_color-mix(in_oklab,var(--foreground)_34%,transparent)]">
          <p className="text-[12px] font-semibold uppercase tracking-[0.22em] text-[#6d7a77]">
            Configuration Preview
          </p>

          <div className="mt-7 space-y-4">
            {previewRows.map((item) => (
              <div
                key={item.label}
                className="flex items-start justify-between gap-4 border-b border-border/60 pb-4 last:border-b-0 last:pb-0"
              >
                <p className="pt-1 text-sm text-[#3d4947]">{item.label}</p>
                {item.chip ? (
                  <span className="max-w-[220px] rounded-md bg-[color:color-mix(in_oklab,var(--brand-teal)_10%,white)] px-2.5 py-1 text-right text-xs font-semibold text-[var(--brand-teal)]">
                    {item.value}
                  </span>
                ) : (
                  <p
                    className={`max-w-[220px] text-right text-sm font-semibold leading-6 ${
                      item.accent ? 'text-[var(--brand-teal)]' : 'text-[#191c1e]'
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
                  <div className="mt-0.5 flex size-10 items-center justify-center rounded-full bg-white text-[var(--brand-teal)]">
                    <Loader2 className="size-5 animate-spin" />
                  </div>
                  <div className="space-y-1">
                    <p className="text-sm font-semibold text-[#191c1e]">
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
                    <p className="text-sm font-semibold text-[#191c1e]">Transcription task created</p>
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
                  <p className="text-sm font-semibold text-[#191c1e]">Source file ready</p>
                  <p className="truncate text-sm text-[#3d4947]">{file?.name ?? 'Source file selected'}</p>
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
