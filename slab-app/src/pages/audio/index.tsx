import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { toast } from 'sonner';
import useFile, { SelectedFile } from '@/hooks/use-file';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './hooks/use-transcribe';
import useIsTauri from '@/hooks/use-tauri';
import api from '@/lib/api';
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

  const whisperModels = useMemo(
    () => (catalogModels ?? []).filter((model) => model.backend_ids.includes(WHISPER_BACKEND_ID)),
    [catalogModels]
  );

  const isWhisperVadModel = (model: {
    display_name: string;
    repo_id: string;
    filename: string;
    is_vad_model?: boolean;
  }): boolean => {
    if (typeof model.is_vad_model === 'boolean') {
      return model.is_vad_model;
    }

    const haystack = `${model.display_name} ${model.repo_id} ${model.filename}`.toLowerCase();
    return (
      haystack.includes(' silero') ||
      haystack.includes('silero ') ||
      haystack.includes('-vad') ||
      haystack.includes('_vad') ||
      haystack.includes(' vad') ||
      haystack.includes('vad ') ||
      haystack.endsWith('vad')
    );
  };

  const whisperTranscribeModels = useMemo(
    () => whisperModels.filter((model) => !isWhisperVadModel(model)),
    [whisperModels]
  );

  const whisperVadModels = useMemo(
    () => whisperModels.filter((model) => isWhisperVadModel(model)),
    [whisperModels]
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

  const pendingTaskIdOf = (model: unknown): string | null => {
    if (typeof model !== 'object' || model === null) return null;
    const pendingTaskId = (model as { pending_task_id?: string | null }).pending_task_id;
    if (typeof pendingTaskId !== 'string') return null;
    const trimmed = pendingTaskId.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

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
    const models = refreshed.data ?? [];
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

    if (!model.backend_ids.includes(WHISPER_BACKEND_ID)) {
      throw new Error(`Selected model does not support ${WHISPER_BACKEND_ID}`);
    }

    if (model.local_path) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    let taskId = pendingTaskIdOf(model);
    if (!taskId) {
      const downloadResponse = await downloadModelMutation.mutateAsync({
        body: {
          backend_id: WHISPER_BACKEND_ID,
          model_id: modelId,
        },
      });
      taskId = extractTaskId(downloadResponse);
    }

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
    if (!isWhisperVadModel(model)) {
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

  return (
    <div className="h-full overflow-y-auto">
      <div className="container mx-auto px-4 py-8 space-y-8">
        <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>Transcription Setup</CardTitle>
          <CardDescription>
            Choose a whisper model and file. You can optionally enable VAD and configure advanced whisper decode parameters. Missing model files are downloaded automatically. Worker count comes from Settings configuration.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {catalogModelsError && (
            <Alert variant="destructive">
              <AlertTitle>Model Catalog Error</AlertTitle>
              <AlertDescription>
                {(catalogModelsError as any)?.message || 'Failed to load model catalog. Please check server status.'}
              </AlertDescription>
            </Alert>
          )}

          {transcribe?.isError && (
            <Alert variant="destructive">
              <AlertTitle>Error</AlertTitle>
              <AlertDescription>
                {(transcribe?.error as any)?.error || 'Failed to create transcription task. Please retry.'}
              </AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>Whisper Model</Label>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
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
              <SelectTrigger>
                <SelectValue placeholder={catalogModelsLoading ? 'Loading models...' : 'Select whisper model'} />
              </SelectTrigger>
              <SelectContent>
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
            {selectedModel ? (
              <p className="text-xs text-muted-foreground">
                {selectedModel.local_path
                  ? 'Downloaded locally.'
                  : pendingTaskIdOf(selectedModel)
                    ? 'Download task is running. It will be reused automatically.'
                    : 'Not downloaded yet. It will be downloaded automatically before transcription.'}
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                {catalogModelsLoading
                  ? 'Loading model catalog...'
                  : 'No Whisper transcription model available. Please add one in Settings first.'}
              </p>
            )}
          </div>

          <div className="space-y-3 rounded-md border p-3">
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-1">
                <Label htmlFor="enable-vad">Enable VAD</Label>
                <p className="text-xs text-muted-foreground">
                  Optional Voice Activity Detection before decoding. Leave parameter fields empty to use whisper.cpp defaults.
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
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label>VAD Model</Label>
                  <Select
                    value={selectedVadModelId}
                    onValueChange={setSelectedVadModelId}
                    disabled={!isTauri || isBusy || whisperVadModels.length === 0}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder={catalogModelsLoading ? 'Loading models...' : 'Select VAD model'} />
                    </SelectTrigger>
                    <SelectContent>
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
                  {selectedVadModel ? (
                    <p className="text-xs text-muted-foreground">
                      {selectedVadModel.local_path
                        ? 'Downloaded locally.'
                        : pendingTaskIdOf(selectedVadModel)
                          ? 'Download task is running. It will be reused automatically.'
                          : 'Not downloaded yet. It will be downloaded automatically before transcription.'}
                    </p>
                  ) : (
                    <p className="text-xs text-muted-foreground">
                      Select a dedicated VAD model (for example files/repo names containing `vad` or `silero`).
                    </p>
                  )}
                </div>

                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <div className="space-y-1">
                    <Label htmlFor="vad-threshold">Threshold</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="vad-min-speech-duration">Min Speech (ms)</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="vad-min-silence-duration">Min Silence (ms)</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="vad-max-speech-duration">Max Speech (s)</Label>
                    <Input
                      id="vad-max-speech-duration"
                      type="number"
                      inputMode="decimal"
                      min={0}
                      step={0.1}
                      value={vadMaxSpeechDurationS}
                      onChange={(e) => setVadMaxSpeechDurationS(e.target.value)}
                      placeholder="(default: no limit)"
                      disabled={isBusy}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="vad-speech-pad">Speech Pad (ms)</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="vad-samples-overlap">Samples Overlap (s)</Label>
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
                    />
                  </div>
                </div>
              </div>
            )}
          </div>

          <div className="space-y-3 rounded-md border p-3">
            <div className="flex items-start justify-between gap-4">
              <div className="space-y-1">
                <Label htmlFor="show-decode-options">Advanced Decode Options</Label>
                <p className="text-xs text-muted-foreground">
                  Optional whisper.cpp decoding params (temperature, thresholds, timestamps, and segmentation behavior).
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
              <div className="space-y-4">
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <div className="space-y-1">
                    <Label htmlFor="decode-offset-ms">Offset (ms)</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-duration-ms">Duration (ms)</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-word-thold">Word Threshold</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-max-len">Max Segment Length</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-max-tokens">Max Tokens / Segment</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-temperature">Temperature</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-temperature-inc">Temperature Increment</Label>
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
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-entropy-thold">Entropy Threshold</Label>
                    <Input
                      id="decode-entropy-thold"
                      type="number"
                      inputMode="decimal"
                      step={0.01}
                      value={decodeEntropyThold}
                      onChange={(e) => setDecodeEntropyThold(e.target.value)}
                      placeholder="2.40"
                      disabled={isBusy}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-logprob-thold">Logprob Threshold</Label>
                    <Input
                      id="decode-logprob-thold"
                      type="number"
                      inputMode="decimal"
                      step={0.01}
                      value={decodeLogprobThold}
                      onChange={(e) => setDecodeLogprobThold(e.target.value)}
                      placeholder="-1.00"
                      disabled={isBusy}
                    />
                  </div>
                  <div className="space-y-1">
                    <Label htmlFor="decode-no-speech-thold">No Speech Threshold</Label>
                    <Input
                      id="decode-no-speech-thold"
                      type="number"
                      inputMode="decimal"
                      step={0.01}
                      value={decodeNoSpeechThold}
                      onChange={(e) => setDecodeNoSpeechThold(e.target.value)}
                      placeholder="0.60"
                      disabled={isBusy}
                    />
                  </div>
                </div>

                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-no-context">No Context</Label>
                    <Switch
                      id="decode-no-context"
                      checked={decodeNoContext}
                      onCheckedChange={setDecodeNoContext}
                      disabled={isBusy}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-no-timestamps">No Timestamps</Label>
                    <Switch
                      id="decode-no-timestamps"
                      checked={decodeNoTimestamps}
                      onCheckedChange={setDecodeNoTimestamps}
                      disabled={isBusy}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-token-timestamps">Token Timestamps</Label>
                    <Switch
                      id="decode-token-timestamps"
                      checked={decodeTokenTimestamps}
                      onCheckedChange={setDecodeTokenTimestamps}
                      disabled={isBusy}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-split-on-word">Split On Word</Label>
                    <Switch
                      id="decode-split-on-word"
                      checked={decodeSplitOnWord}
                      onCheckedChange={setDecodeSplitOnWord}
                      disabled={isBusy}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-suppress-nst">Suppress NST</Label>
                    <Switch
                      id="decode-suppress-nst"
                      checked={decodeSuppressNst}
                      onCheckedChange={setDecodeSuppressNst}
                      disabled={isBusy}
                    />
                  </div>
                  <div className="flex items-center justify-between rounded-md border p-3">
                    <Label htmlFor="decode-tdrz-enable">Enable TinyDiarize</Label>
                    <Switch
                      id="decode-tdrz-enable"
                      checked={decodeTdrzEnable}
                      onCheckedChange={setDecodeTdrzEnable}
                      disabled={isBusy}
                    />
                  </div>
                </div>
              </div>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor={isTauri ? undefined : 'file'}>File</Label>
            {isTauri ? (
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleTauriFileSelect()}
                disabled={isBusy}
              >
                {file ? 'Change File' : 'Choose File'}
              </Button>
            ) : (
              <Input
                id="file"
                type="file"
                accept="audio/*,video/*"
                onChange={handleFileChange}
                disabled={isBusy || !isTauri}
              />
            )}
            {file && (
              <p className="text-sm text-muted-foreground">
                Selected: {file.name}
              </p>
            )}
          </div>

          {isBusy && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>
                {preparingStage === 'prepare'
                  ? 'Preparing selected model...'
                  : 'Processing transcription request...'}
              </p>
              {taskId && preparingStage !== 'prepare' && (
                <p className="text-xs text-muted-foreground">Task ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleTranscribe}
            disabled={!isTauri || !file || !selectedModelId || isBusy || (enableVad && !selectedVadModelId)}
          >
            {preparingStage === 'prepare'
              ? 'Preparing Model...'
              : preparingStage === 'transcribe' || transcribe?.isPending
                ? 'Processing...'
                : 'Start Transcription'}
          </Button>
        </CardFooter>
        </Card>
      </div>
    </div>
  );
}
