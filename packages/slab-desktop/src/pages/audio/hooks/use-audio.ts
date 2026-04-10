import { useEffect, useMemo, useRef, useState } from 'react';
import type { ChangeEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import useFile, { type SelectedFile } from '@/hooks/use-file';
import useIsTauri from '@/hooks/use-tauri';
import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import api from '@/lib/api';
import { modelSupportsCapability, toCatalogModelList, type CatalogModel } from '@/lib/api/models';
import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import { HEADER_SELECT_KEYS } from '@/layouts/header-controls';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './use-transcribe';
import {
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  type PreparingStage,
} from '../const';

export function useAudio() {
  const navigate = useNavigate();
  const isTauri = useIsTauri();
  usePageHeader(PAGE_HEADER_META.audio);

  const [file, setFile] = useState<SelectedFile | null>(null);
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
    data: transcriptionCatalogModels,
    isLoading: transcriptionModelsLoading,
    error: transcriptionModelsError,
    refetch: refetchTranscriptionModels,
  } = api.useQuery('get', '/v1/models', {
    params: {
      query: {
        capability: 'audio_transcription',
      },
    },
  });
  const {
    data: vadCatalogModels,
    isLoading: vadModelsLoading,
    error: vadModelsError,
    refetch: refetchVadModels,
  } = api.useQuery('get', '/v1/models', {
    params: {
      query: {
        capability: 'audio_vad',
      },
    },
  });
  const downloadModelMutation = api.useMutation('post', '/v1/models/download');
  const loadModelMutation = api.useMutation('post', '/v1/models/load');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');

  const whisperTranscribeModels = useMemo(
    () => toCatalogModelList(transcriptionCatalogModels).filter((model) => model.kind === 'local'),
    [transcriptionCatalogModels],
  );

  const whisperVadModels = useMemo(
    () => toCatalogModelList(vadCatalogModels).filter((model) => model.kind === 'local'),
    [vadCatalogModels],
  );

  const audioModels = useMemo(() => {
    const merged = new Map<string, CatalogModel>();
    whisperTranscribeModels.forEach((model) => {
      merged.set(model.id, model);
    });
    whisperVadModels.forEach((model) => {
      merged.set(model.id, model);
    });
    return Array.from(merged.values());
  }, [whisperTranscribeModels, whisperVadModels]);
  const catalogModelsLoading = transcriptionModelsLoading || vadModelsLoading;
  const catalogModelsError = transcriptionModelsError ?? vadModelsError;
  const { value: selectedModelId, setValue: setSelectedModelId } = usePersistedHeaderSelect({
    key: HEADER_SELECT_KEYS.audioModel,
    options: whisperTranscribeModels,
    isLoading: catalogModelsLoading,
  });

  const selectedModel = useMemo(
    () => whisperTranscribeModels.find((model) => model.id === selectedModelId),
    [whisperTranscribeModels, selectedModelId],
  );

  const selectedVadModel = useMemo(
    () => whisperVadModels.find((model) => model.id === selectedVadModelId),
    [whisperVadModels, selectedVadModelId],
  );

  const isBusy =
    Boolean(preparingStage) ||
    transcribe.isPending ||
    loadModelMutation.isPending ||
    downloadModelMutation.isPending;
  const headerModelPicker = useMemo(
    () => ({
      type: 'select' as const,
      value: selectedModelId,
      options: whisperTranscribeModels.map((model) => ({
        id: model.id,
        label: model.display_name,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: 'Whisper Models',
      placeholder: 'Select model',
      loading: catalogModelsLoading,
      disabled: catalogModelsLoading || isBusy || whisperTranscribeModels.length === 0,
      emptyLabel: 'No whisper models',
    }),
    [catalogModelsLoading, isBusy, selectedModelId, setSelectedModelId, whisperTranscribeModels],
  );
  const webFileInputRef = useRef<HTMLInputElement>(null);

  usePageHeaderControl(headerModelPicker);

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
    const tid =
      (payload as { operation_id?: unknown }).operation_id ??
      (payload as { task_id?: unknown }).task_id;
    if (typeof tid !== 'string') return null;
    const trimmed = tid.trim();
    return trimmed.length > 0 ? trimmed : null;
  };

  const waitForTaskToFinish = async (tid: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      const task = (await getTaskMutation.mutateAsync({
        params: {
          path: { id: tid },
        },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (task.status === 'failed' || task.status === 'cancelled' || task.status === 'interrupted') {
        throw new Error(task.error_msg ?? `Task ${tid} ended with status: ${task.status}`);
      }

      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error('Model download timed out');
  };

  const refreshCatalogAndFindModel = async (modelId: string) => {
    const [transcriptionRefresh, vadRefresh] = await Promise.all([
      refetchTranscriptionModels(),
      refetchVadModels(),
    ]);
    const models = [
      ...toCatalogModelList(transcriptionRefresh.data),
      ...toCatalogModelList(vadRefresh.data),
    ];
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
    options: { min?: number; max?: number; exclusiveMin?: number } = {},
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
    modelId: string,
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = audioModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error('Selected model does not exist in catalog');
    }

    if (model.kind !== 'local') {
      throw new Error('Selected model is not a local audio model.');
    }

    if (model.local_path) {
      return { modelPath: model.local_path, downloadedNow: false };
    }

    const downloadResponse = await downloadModelMutation.mutateAsync({
      body: {
        model_id: modelId,
      },
    });
    const tid = extractTaskId(downloadResponse);

    if (!tid) {
      throw new Error('Failed to start model download task');
    }

    await waitForTaskToFinish(tid);

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

    const { downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(`Downloaded ${model.display_name}`);
    }

    await loadModelMutation.mutateAsync({
      body: {
        model_id: selectedModelId,
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
    if (!modelSupportsCapability(model, 'audio_vad')) {
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
      0,
    );
    const minSilenceDurationMs = parseOptionalInt(
      vadMinSilenceDurationMs,
      'VAD min silence duration (ms)',
      0,
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

  const handleFileChange = async (e: ChangeEvent<HTMLInputElement>) => {
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
          onClick: () => navigate('/task'),
        },
      });
    } catch (err: unknown) {
      const anyErr = err as { message?: string; error?: string } | null;
      toast.error('Failed to create transcription task.', {
        description: anyErr?.message || anyErr?.error || 'Unknown error',
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

  return {
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
  };
}
