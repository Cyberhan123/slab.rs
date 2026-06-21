import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ChangeEvent } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import useFile, { type SelectedFile } from '@/hooks/use-file';
import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import useIsTauri from '@/hooks/use-tauri';
import api from '@slab/api';
import { modelSupportsCapability, toCatalogModelList } from '@slab/api/models';
import {
  deriveProgress,
  getAudioTranscription,
  type GenerationProgress,
} from '@/lib/media-task-api';
import { getErrorDescription } from '@/lib/error-description';
import {
  useModelConfigDocumentQuery,
  type ModelConfigDocumentResponse,
} from '@/lib/model-config';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { useAudioUiStore } from '@/store/useAudioUiStore';
import { useMediaTaskPolling } from '@/pages/task/hooks/use-media-task-polling';
import {
  extractTaskId,
  isFailedTaskStatus,
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  sleep,
} from '@/pages/task/utils';
import {
  BUNDLED_VAD_MODEL_ID,
  type PreparingStage,
} from '../const';
import {
  areAudioTranscriptionControlValuesEqual,
  buildAudioTranscriptionControlsFromModelConfig,
  createDefaultAudioTranscriptionControls,
  normalizeAudioTranscriptionControls,
  type AudioTranscriptionControls,
} from '../lib/audio-transcription-controls';
import { parseOptionalFloat, parseOptionalInt } from '../lib/audio-value-parsing';
import { findBundledVadArtifact } from '../lib/audio-vad-models';
import { useAudioHistory } from './use-audio-history';
import { useAudioModelCatalog } from './use-audio-model-catalog';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './use-transcribe';

export function useAudio() {
  const { t } = useTranslation();
  const isTauri = useIsTauri();
  usePageHeader({
    icon: PAGE_HEADER_META.audio.icon,
    title: t('pages.audio.header.title'),
    subtitle: t('pages.audio.header.subtitle'),
  });

  const [file, setFile] = useState<SelectedFile | null>(null);
  const [preparingStage, setPreparingStage] = useState<PreparingStage>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [transcriptionPhase, setTranscriptionPhase] = useState<'idle' | 'polling' | 'fetchingResult'>('idle');
  const [generationProgress, setGenerationProgress] = useState<GenerationProgress | null>(null);
  const generationProgressRef = useRef<GenerationProgress | null>(null);
  const {
    history,
    historyDialogOpen,
    historyError,
    historyLoading,
    openHistoryDetail,
    refreshHistory,
    selectedHistoryTask,
    setHistoryDialogOpen,
    setSelectedHistoryTask,
    showHistoryTask,
  } = useAudioHistory();

  const { handleFile } = useFile();
  const transcribe = useTranscribe();
  const hasHydrated = useAudioUiStore((state) => state.hasHydrated);
  const modelControlOverrides = useAudioUiStore((state) => state.modelControlOverrides);
  const setModelControlOverrides = useAudioUiStore((state) => state.setModelControlOverrides);
  const clearModelControlOverrides = useAudioUiStore((state) => state.clearModelControlOverrides);
  const {
    audioModels,
    catalogModelsError,
    catalogModelsLoading,
    refetchTranscriptionModels,
    refetchVadModels,
    selectedModel,
    selectedModelId,
    setSelectedModelId,
    whisperTranscribeModels,
    whisperVadModels,
  } = useAudioModelCatalog();
  const downloadModelMutation = api.useMutation('post', '/v1/models/download', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const loadModelMutation = api.useMutation('post', '/v1/models/load', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const cancelTaskMutation = api.useMutation('post', '/v1/tasks/{id}/cancel', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const isTranscriptionPolling = transcriptionPhase === 'polling';
  const isTranscriptionFetchingResult = transcriptionPhase === 'fetchingResult';
  const toPollingErrorMessage = useCallback(
    (message: string) => t('pages.audio.toast.pollingError', { message }),
    [t],
  );
  const {
    taskStatus,
    taskStatusUpdatedAt,
  } = useMediaTaskPolling({
    enabled: isTranscriptionPolling,
    intervalMs: MODEL_DOWNLOAD_POLL_INTERVAL_MS,
    pollingErrorToastId: 'audio-transcription-polling-error',
    taskId,
    toPollingErrorMessage,
  });

  const {
    data: selectedModelConfigDocument,
    error: selectedModelConfigError,
    refetch: refetchSelectedModelConfigDocument,
  } = useModelConfigDocumentQuery(selectedModelId || null, {
    enabled: isTauri && hasHydrated && Boolean(selectedModelId),
  });
  const selectedModelPresetControls = useMemo(() => {
    if (!selectedModelId || !selectedModelConfigDocument) {
      return createDefaultAudioTranscriptionControls();
    }

    return buildAudioTranscriptionControlsFromModelConfig(selectedModelConfigDocument);
  }, [selectedModelConfigDocument, selectedModelId]);
  const controlOverrides =
    selectedModelId && hasHydrated ? modelControlOverrides[selectedModelId] : undefined;
  const controls = useMemo(
    () =>
      normalizeAudioTranscriptionControls({
        ...selectedModelPresetControls,
        ...controlOverrides,
      }),
    [controlOverrides, selectedModelPresetControls],
  );
  const {
    enableVad,
    selectedVadModelId: overriddenVadModelId,
    vadThreshold,
    vadMinSpeechDurationMs,
    vadMinSilenceDurationMs,
    vadMaxSpeechDurationS,
    vadSpeechPadMs,
    vadSamplesOverlap,
    showDecodeOptions,
    decodeOffsetMs,
    decodeDurationMs,
    decodeWordThold,
    decodeMaxLen,
    decodeMaxTokens,
    decodeTemperature,
    decodeTemperatureInc,
    decodeEntropyThold,
    decodeLogprobThold,
    decodeNoSpeechThold,
    decodeNoContext,
    decodeNoTimestamps,
    decodeTokenTimestamps,
    decodeSplitOnWord,
    decodeSuppressNst,
    decodeTdrzEnable,
    language,
    prompt,
    detectLanguage,
  } = controls;

  const bundledVadArtifact = useMemo(
    () => findBundledVadArtifact(selectedModelConfigDocument),
    [selectedModelConfigDocument],
  );
  const hasBundledVad = Boolean(bundledVadArtifact?.value);
  const selectedVadModelId = useMemo(() => {
    if (!enableVad) {
      return overriddenVadModelId;
    }

    if (
      overriddenVadModelId === BUNDLED_VAD_MODEL_ID &&
      hasBundledVad
    ) {
      return overriddenVadModelId;
    }

    if (
      overriddenVadModelId &&
      overriddenVadModelId !== BUNDLED_VAD_MODEL_ID &&
      whisperVadModels.some((model) => model.id === overriddenVadModelId)
    ) {
      return overriddenVadModelId;
    }

    if (hasBundledVad) {
      return BUNDLED_VAD_MODEL_ID;
    }

    return whisperVadModels[0]?.id ?? '';
  }, [enableVad, hasBundledVad, overriddenVadModelId, whisperVadModels]);
  const isUsingBundledVad =
    enableVad && selectedVadModelId === BUNDLED_VAD_MODEL_ID && hasBundledVad;
  const selectedVadModel = useMemo(
    () =>
      selectedVadModelId === BUNDLED_VAD_MODEL_ID
        ? undefined
        : whisperVadModels.find((model) => model.id === selectedVadModelId),
    [selectedVadModelId, whisperVadModels],
  );

  const isBusy =
    Boolean(preparingStage) ||
    transcriptionPhase !== 'idle' ||
    transcribe.isPending ||
    loadModelMutation.isPending ||
    downloadModelMutation.isPending ||
    cancelTaskMutation.isPending;
  const headerModelPicker = useMemo(
    () => ({
      type: 'select' as const,
      value: selectedModelId,
      options: whisperTranscribeModels.map((model) => ({
        id: model.id,
        label: model.display_name,
      })),
      onValueChange: setSelectedModelId,
      groupLabel: t('pages.audio.modelPicker.groupLabel'),
      placeholder: t('pages.audio.modelPicker.placeholder'),
      loading: catalogModelsLoading,
      disabled: catalogModelsLoading || isBusy || whisperTranscribeModels.length === 0,
      emptyLabel: t('pages.audio.modelPicker.emptyLabel'),
    }),
    [catalogModelsLoading, isBusy, selectedModelId, setSelectedModelId, t, whisperTranscribeModels],
  );
  const webFileInputRef = useRef<HTMLInputElement>(null);

  usePageHeaderControl(headerModelPicker);

  useEffect(() => {
    if (!selectedModelId || !selectedModelConfigError) {
      return;
    }

    console.warn(
      `Failed to load audio preset defaults for model '${selectedModelId}'.`,
      selectedModelConfigError,
    );
  }, [selectedModelConfigError, selectedModelId]);

  const updateControl = useCallback(
    <K extends keyof AudioTranscriptionControls>(
      key: K,
      value: AudioTranscriptionControls[K],
    ) => {
      const normalizedValue = normalizeAudioTranscriptionControls({
        ...controls,
        [key]: value,
      })[key];

      if (!selectedModelId) {
        return;
      }

      const nextOverrides = { ...controlOverrides };
      if (
        areAudioTranscriptionControlValuesEqual(
          normalizedValue,
          selectedModelPresetControls[key],
        )
      ) {
        delete nextOverrides[key];
      } else {
        nextOverrides[key] = normalizedValue;
      }

      if (Object.keys(nextOverrides).length === 0) {
        clearModelControlOverrides(selectedModelId);
        return;
      }

      setModelControlOverrides(selectedModelId, nextOverrides);
    },
    [
      clearModelControlOverrides,
      controlOverrides,
      controls,
      selectedModelId,
      selectedModelPresetControls,
      setModelControlOverrides,
    ],
  );

  const waitForTaskToFinish = async (tid: string) => {
    const deadline = Date.now() + MODEL_DOWNLOAD_TIMEOUT_MS;

    while (Date.now() < deadline) {
      // eslint-disable-next-line no-await-in-loop
      const task = (await getTaskMutation.mutateAsync({
        params: {
          path: { id: tid },
        },
      })) as { status: string; error_msg?: string | null };

      if (task.status === 'succeeded') {
        return;
      }

      if (isFailedTaskStatus(task.status)) {
        throw new Error(
          task.error_msg ??
            t('pages.hub.error.taskEndedWithStatus', {
              taskId: tid,
              status: task.status,
            }),
        );
      }

      // eslint-disable-next-line no-await-in-loop
      await sleep(MODEL_DOWNLOAD_POLL_INTERVAL_MS);
    }

    throw new Error(t('pages.audio.error.downloadTimedOut'));
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

  const ensureDownloadedModelPath = async (
    modelId: string,
  ): Promise<{ modelPath: string; downloadedNow: boolean }> => {
    let model = audioModels.find((item) => item.id === modelId);
    if (!model) {
      model = await refreshCatalogAndFindModel(modelId);
    }

    if (!model) {
      throw new Error(t('pages.audio.error.selectedModelMissingGeneric'));
    }

    if (model.kind !== 'local') {
      throw new Error(t('pages.audio.error.selectedModelNotLocal'));
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
      throw new Error(t('pages.audio.error.startDownloadFailed'));
    }

    await waitForTaskToFinish(tid);

    const refreshedModel = await refreshCatalogAndFindModel(modelId);
    if (!refreshedModel?.local_path) {
      throw new Error(t('pages.audio.error.missingDownloadedPath'));
    }

    return { modelPath: refreshedModel.local_path, downloadedNow: true };
  };

  const prepareSelectedModel = async (): Promise<string> => {
    if (!selectedModelId) {
      throw new Error(t('pages.audio.error.selectModelFirst'));
    }

    const model = whisperTranscribeModels.find((item) => item.id === selectedModelId);
    if (!model) {
      throw new Error(t('pages.audio.error.selectedModelMissing'));
    }

    const { downloadedNow } = await ensureDownloadedModelPath(selectedModelId);

    if (downloadedNow) {
      toast.success(t('pages.audio.toast.downloaded', { model: model.display_name }));
    }

    await loadModelMutation.mutateAsync({
      body: {
        model_id: selectedModelId,
      },
    });

    return model.display_name;
  };

  const clearTranscriptionTask = useCallback(() => {
    generationProgressRef.current = null;
    setGenerationProgress(null);
    setTranscriptionPhase('idle');
    setTaskId(null);
  }, []);

  useEffect(() => {
    if (!isTranscriptionPolling || !taskId || taskStatusUpdatedAt === 0) {
      return;
    }

    const nextProgress = deriveProgress(
      taskStatus?.progress ?? null,
      generationProgressRef.current,
      taskStatusUpdatedAt,
    );
    generationProgressRef.current = nextProgress;
    setGenerationProgress(nextProgress);

    if (!taskStatus) {
      return;
    }

    if (taskStatus.status === 'succeeded') {
      setTranscriptionPhase('fetchingResult');
      return;
    }

    if (taskStatus.status === 'failed') {
      toast.error(taskStatus.error_msg ?? t('pages.audio.error.transcriptionFailed'));
      clearTranscriptionTask();
      return;
    }

    if (taskStatus.status === 'cancelled' || taskStatus.status === 'interrupted') {
      toast.success(t('pages.audio.toast.cancelled'));
      clearTranscriptionTask();
      void refreshHistory();
    }
  }, [
    clearTranscriptionTask,
    isTranscriptionPolling,
    refreshHistory,
    taskId,
    taskStatus,
    taskStatusUpdatedAt,
    t,
  ]);

  useEffect(() => {
    if (!isTranscriptionFetchingResult || !taskId) {
      return;
    }

    let cancelled = false;

    const loadResult = async () => {
      try {
        const detail = await getAudioTranscription(taskId);
        if (cancelled) {
          return;
        }

        showHistoryTask(detail);
        toast.success(t('pages.audio.toast.transcriptionReady'));
        await refreshHistory();
      } catch (error) {
        if (cancelled) {
          return;
        }

        toast.error(t('pages.audio.toast.failedToCreateTask'), {
          description: getErrorDescription(error, t('pages.audio.toast.unknownError')),
        });
      } finally {
        if (!cancelled) {
          clearTranscriptionTask();
        }
      }
    };

    void loadResult();

    return () => {
      cancelled = true;
    };
  }, [
    clearTranscriptionTask,
    isTranscriptionFetchingResult,
    refreshHistory,
    showHistoryTask,
    t,
    taskId,
  ]);

  const prepareVadSettings = async (
    modelConfigDocument: ModelConfigDocumentResponse | undefined,
  ): Promise<{ settings: TranscribeVadSettings; modelName: string }> => {
    const bundledArtifact = findBundledVadArtifact(modelConfigDocument);

    let modelPath: string | null = null;
    let modelName = t('pages.audio.vad.bundledFallback');

    if (selectedVadModelId === BUNDLED_VAD_MODEL_ID) {
      modelPath = bundledArtifact?.value?.trim() ?? null;
      modelName = bundledArtifact?.label?.trim() || t('pages.audio.vad.bundledFallback');
      if (!modelPath) {
        throw new Error(t('pages.audio.error.bundledVadMissing'));
      }
    } else {
      if (!selectedVadModelId) {
        throw new Error(t('pages.audio.error.selectDedicatedVadModel'));
      }

      let model = whisperVadModels.find((item) => item.id === selectedVadModelId);
      if (!model) {
        model = await refreshCatalogAndFindModel(selectedVadModelId);
      }
      if (!model) {
        throw new Error(t('pages.audio.error.selectedVadMissing'));
      }
      if (!modelSupportsCapability(model, 'audio_vad')) {
        throw new Error(t('pages.audio.error.selectedModelNotDedicatedVad'));
      }

      const preparedModel = await ensureDownloadedModelPath(selectedVadModelId);
      modelPath = preparedModel.modelPath;
      modelName = model.display_name;
      if (preparedModel.downloadedNow) {
        toast.success(t('pages.audio.toast.downloadedVadModel', { model: model.display_name }));
      }
    }

    const resolvedModelPath = modelPath?.trim();
    if (!resolvedModelPath) {
      throw new Error(t('pages.audio.error.resolveVadPath'));
    }

    const settings: TranscribeVadSettings = {
      enabled: true,
      model_path: resolvedModelPath,
    };

    const threshold = parseOptionalFloat(
      vadThreshold,
      t('pages.audio.validation.labels.vadThreshold'),
      t,
      { min: 0, max: 1 },
    );
    const minSpeechDurationMs = parseOptionalInt(
      vadMinSpeechDurationMs,
      t('pages.audio.validation.labels.vadMinSpeechDurationMs'),
      0,
      t,
    );
    const minSilenceDurationMs = parseOptionalInt(
      vadMinSilenceDurationMs,
      t('pages.audio.validation.labels.vadMinSilenceDurationMs'),
      0,
      t,
    );
    const maxSpeechDurationS = parseOptionalFloat(
      vadMaxSpeechDurationS,
      t('pages.audio.validation.labels.vadMaxSpeechDurationS'),
      t,
      {
        exclusiveMin: 0,
      },
    );
    const speechPadMs = parseOptionalInt(
      vadSpeechPadMs,
      t('pages.audio.validation.labels.vadSpeechPadMs'),
      0,
      t,
    );
    const samplesOverlap = parseOptionalFloat(
      vadSamplesOverlap,
      t('pages.audio.validation.labels.vadSamplesOverlap'),
      t,
      { min: 0 },
    );

    if (threshold !== undefined) settings.threshold = threshold;
    if (minSpeechDurationMs !== undefined) settings.min_speech_duration_ms = minSpeechDurationMs;
    if (minSilenceDurationMs !== undefined) settings.min_silence_duration_ms = minSilenceDurationMs;
    if (maxSpeechDurationS !== undefined) settings.max_speech_duration_s = maxSpeechDurationS;
    if (speechPadMs !== undefined) settings.speech_pad_ms = speechPadMs;
    if (samplesOverlap !== undefined) settings.samples_overlap = samplesOverlap;
    return { settings, modelName };
  };

  const prepareInferenceOptions = (): Omit<TranscribeOptions, 'decode' | 'vad'> | undefined => {
    const next: Omit<TranscribeOptions, 'decode' | 'vad'> = {};
    const trimmedLanguage = language.trim();
    const trimmedPrompt = prompt.trim();

    if (trimmedLanguage) {
      next.language = trimmedLanguage;
    }
    if (trimmedPrompt) {
      next.prompt = trimmedPrompt;
    }
    if (!trimmedLanguage && detectLanguage) {
      next.language = 'auto';
    }

    return Object.keys(next).length > 0 ? next : undefined;
  };

  const prepareDecodeOptions = (): TranscribeOptions['decode'] | undefined => {
    if (!showDecodeOptions) {
      return undefined;
    }

    const decode: NonNullable<TranscribeOptions['decode']> = {};

    const offsetMs = parseOptionalInt(
      decodeOffsetMs,
      t('pages.audio.validation.labels.decodeOffsetMs'),
      0,
      t,
    );
    const durationMs = parseOptionalInt(
      decodeDurationMs,
      t('pages.audio.validation.labels.decodeDurationMs'),
      0,
      t,
    );
    const wordThold = parseOptionalFloat(
      decodeWordThold,
      t('pages.audio.validation.labels.decodeWordThreshold'),
      t,
      { min: 0, max: 1 },
    );
    const maxLen = parseOptionalInt(
      decodeMaxLen,
      t('pages.audio.validation.labels.decodeMaxSegmentLength'),
      0,
      t,
    );
    const maxTokens = parseOptionalInt(
      decodeMaxTokens,
      t('pages.audio.validation.labels.decodeMaxTokensPerSegment'),
      0,
      t,
    );
    const temperature = parseOptionalFloat(
      decodeTemperature,
      t('pages.audio.validation.labels.decodeTemperature'),
      t,
      { min: 0 },
    );
    const temperatureInc = parseOptionalFloat(
      decodeTemperatureInc,
      t('pages.audio.validation.labels.decodeTemperatureIncrement'),
      t,
      { min: 0 },
    );
    const entropyThold = parseOptionalFloat(
      decodeEntropyThold,
      t('pages.audio.validation.labels.decodeEntropyThreshold'),
      t,
    );
    const logprobThold = parseOptionalFloat(
      decodeLogprobThold,
      t('pages.audio.validation.labels.decodeLogprobThreshold'),
      t,
    );
    const noSpeechThold = parseOptionalFloat(
      decodeNoSpeechThold,
      t('pages.audio.validation.labels.decodeNoSpeechThreshold'),
      t,
    );

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
      toast.error(t('pages.audio.error.webUploadNotImplemented'));
      return;
    }

    if (!file) {
      toast.error(t('pages.audio.error.selectFileFirst'));
      return;
    }

    if (!selectedModelId) {
      toast.error(t('pages.audio.error.selectModelFirst'));
      return;
    }

    try {
      clearTranscriptionTask();
      setPreparingStage('prepare');
      const modelName = await prepareSelectedModel();
      const refreshedModelConfigDocument = selectedModelId
        ? (await refetchSelectedModelConfigDocument()).data ?? selectedModelConfigDocument
        : selectedModelConfigDocument;
      let vadDescription = t('pages.audio.summary.vadOff');
      let decodeDescription = t('pages.audio.summary.decodeDefault');
      let transcribeOptions: TranscribeOptions | undefined;

      const inferenceOptions = prepareInferenceOptions();
      if (inferenceOptions) {
        transcribeOptions = inferenceOptions;
      }

      if (enableVad) {
        const preparedVad = await prepareVadSettings(refreshedModelConfigDocument);
        transcribeOptions = { ...transcribeOptions, vad: preparedVad.settings };
        vadDescription = t('pages.audio.summary.vadOn', { model: preparedVad.modelName });
      }

      const decodeOptions = prepareDecodeOptions();
      if (decodeOptions) {
        transcribeOptions = { ...transcribeOptions, decode: decodeOptions };
        decodeDescription = t('pages.audio.summary.decodeCustom');
      }

      transcribeOptions = { ...transcribeOptions, model_id: selectedModelId };

      setPreparingStage('transcribe');
      const result = await transcribe.handleTranscribe(file.file, transcribeOptions);
      setTaskId(result.operation_id);
      const initialProgress = deriveProgress(null, null, Date.now());
      generationProgressRef.current = initialProgress;
      setGenerationProgress(initialProgress);
      setTranscriptionPhase('polling');

      toast.success(t('pages.audio.toast.taskCreated'), {
        description: t('pages.audio.toast.taskCreatedDescription', {
          id: result.operation_id,
          model: modelName,
          vad: vadDescription,
          decode: decodeDescription,
        }),
      });
    } catch (err: unknown) {
      toast.error(t('pages.audio.toast.failedToCreateTask'), {
        description: getErrorDescription(err, t('pages.audio.toast.unknownError')),
      });
    } finally {
      setPreparingStage(null);
    }
  };

  const handleCancelTranscription = useCallback(async () => {
    if (!taskId) {
      clearTranscriptionTask();
      return;
    }

    try {
      await cancelTaskMutation.mutateAsync({
        params: {
          path: { id: taskId },
        },
      });
    } catch (error) {
      toast.error(t('pages.audio.toast.cancelFailed'), {
        description: getErrorDescription(error, t('pages.audio.toast.unknownError')),
      });
    }
  }, [cancelTaskMutation, clearTranscriptionTask, t, taskId]);

  const canStartTranscription =
    isTauri &&
    Boolean(file) &&
    Boolean(selectedModelId) &&
    !isBusy &&
    (!enableVad || hasBundledVad || Boolean(selectedVadModelId));

  const previewRows = [
    {
      label: t('pages.audio.preview.rows.model'),
      value: selectedModel?.display_name ?? t('pages.audio.preview.values.notSelected'),
      accent: Boolean(selectedModel),
      chip: true,
    },
    {
      label: t('pages.audio.preview.rows.source'),
      value: file?.name ?? t('pages.audio.preview.values.awaitingUpload'),
      accent: Boolean(file),
      chip: false,
    },
    {
      label: t('pages.audio.preview.rows.vadMode'),
      value: enableVad
        ? isUsingBundledVad
          ? t('pages.audio.preview.values.activeBundled', {
              model: bundledVadArtifact?.label ?? t('pages.audio.vad.bundledFallback'),
            })
          : selectedVadModel?.display_name
            ? t('pages.audio.preview.values.activeModel', { model: selectedVadModel.display_name })
            : t('pages.audio.preview.values.active')
        : t('pages.audio.preview.values.inactive'),
      accent: enableVad,
      chip: false,
    },
    {
      label: t('pages.audio.preview.rows.decode'),
      value: showDecodeOptions
        ? t('pages.audio.preview.values.customProfile')
        : t('pages.audio.preview.values.defaultProfile'),
      accent: showDecodeOptions,
      chip: false,
    },
  ];

  return {
    bundledVadLabel: bundledVadArtifact?.label ?? t('pages.audio.vad.bundledFallback'),
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
    generationProgress,
    handleFileChange,
    handleCancelTranscription,
    handleTauriFileSelect,
    handleTranscribe,
    hasBundledVad,
    history,
    historyDialogOpen,
    historyError,
    historyLoading,
    isBusy,
    isCancellingTranscription: cancelTaskMutation.isPending,
    isTauri,
    isTranscriptionRunning: transcriptionPhase !== 'idle',
    isUsingBundledVad,
    openHistoryDetail,
    preparingStage,
    previewRows,
    selectedHistoryTask,
    selectedVadModel,
    selectedVadModelId,
    setDecodeEntropyThold: (value: string) => updateControl('decodeEntropyThold', value),
    setDecodeDurationMs: (value: string) => updateControl('decodeDurationMs', value),
    setDecodeLogprobThold: (value: string) => updateControl('decodeLogprobThold', value),
    setDecodeMaxLen: (value: string) => updateControl('decodeMaxLen', value),
    setDecodeMaxTokens: (value: string) => updateControl('decodeMaxTokens', value),
    setDecodeNoContext: (value: boolean) => updateControl('decodeNoContext', value),
    setDecodeNoSpeechThold: (value: string) => updateControl('decodeNoSpeechThold', value),
    setDecodeNoTimestamps: (value: boolean) => updateControl('decodeNoTimestamps', value),
    setDecodeOffsetMs: (value: string) => updateControl('decodeOffsetMs', value),
    setDecodeSplitOnWord: (value: boolean) => updateControl('decodeSplitOnWord', value),
    setDecodeSuppressNst: (value: boolean) => updateControl('decodeSuppressNst', value),
    setDecodeTdrzEnable: (value: boolean) => updateControl('decodeTdrzEnable', value),
    setDecodeTemperature: (value: string) => updateControl('decodeTemperature', value),
    setDecodeTemperatureInc: (value: string) => updateControl('decodeTemperatureInc', value),
    setDecodeTokenTimestamps: (value: boolean) => updateControl('decodeTokenTimestamps', value),
    setDecodeWordThold: (value: string) => updateControl('decodeWordThold', value),
    setEnableVad: (value: boolean) => updateControl('enableVad', value),
    setHistoryDialogOpen,
    setSelectedVadModelId: (value: string) => updateControl('selectedVadModelId', value),
    setSelectedHistoryTask,
    setShowDecodeOptions: (value: boolean) => updateControl('showDecodeOptions', value),
    setVadMaxSpeechDurationS: (value: string) => updateControl('vadMaxSpeechDurationS', value),
    setVadMinSilenceDurationMs: (value: string) =>
      updateControl('vadMinSilenceDurationMs', value),
    setVadMinSpeechDurationMs: (value: string) =>
      updateControl('vadMinSpeechDurationMs', value),
    setVadSamplesOverlap: (value: string) => updateControl('vadSamplesOverlap', value),
    setVadSpeechPadMs: (value: string) => updateControl('vadSpeechPadMs', value),
    setVadThreshold: (value: string) => updateControl('vadThreshold', value),
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
