import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ChangeEvent } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import useFile, { type SelectedFile } from '@slab/ui/hooks/use-file';
import { useHeader } from '@slab/ui/hooks/use-header';
import useIsTauri from '@slab/ui/hooks/use-tauri';
import api from '@slab/api';
import { modelSupportsCapability } from '@slab/ui/hooks/use-ai-model';
import {
  deriveProgress,
  getAudioTranscription,
  type AudioTranscriptionTask,
  type GenerationProgress,
} from '@slab/core/media/task-api';
import { getErrorDescription } from '@slab/core/api/error-description';
import {
  useModelConfigDocumentQuery,
  type ModelConfigDocumentResponse,
} from '@slab/core/models/config';
import { useAudioUiStore } from '@slab/ui/store/useAudioUiStore';
import { useMediaTaskPolling } from '@slab/ui/pages/task/hooks/use-media-task-polling';
import { MODEL_DOWNLOAD_POLL_INTERVAL_MS } from '@slab/ui/pages/task/utils';
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
import { prepareDecodeOptions, prepareInferenceOptions } from '../lib/audio-transcription-options';
import { findBundledVadArtifact } from '../lib/audio-vad-models';
import { useAudioHistory } from './use-audio-history';
import { useAudioModelCatalog } from './use-audio-model-catalog';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './use-transcribe';

export function useAudio() {
  const { t } = useTranslation();
  const isTauri = useIsTauri();

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
    catalogModelsError,
    catalogModelsLoading,
    ensureDownloadedTranscriptionModel,
    ensureDownloadedVadModel,
    loadTranscriptionModel,
    modelLifecycleBusy,
    selectedModel,
    selectedModelId,
    setSelectedModelId,
    whisperTranscribeModels,
    whisperVadModels,
  } = useAudioModelCatalog();
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
    modelLifecycleBusy ||
    cancelTaskMutation.isPending;
  const headerModelPicker = useMemo(
    () => ({
      value: selectedModelId,
      options: whisperTranscribeModels.map((model) => ({
        id: model.id,
        label: model.display_name,
      })),
      onChange: setSelectedModelId,
      groupLabel: t('pages.audio.modelPicker.groupLabel'),
      placeholder: t('pages.audio.modelPicker.placeholder'),
      loading: catalogModelsLoading,
      disabled: catalogModelsLoading || isBusy || whisperTranscribeModels.length === 0,
      emptyLabel: t('pages.audio.modelPicker.emptyLabel'),
    }),
    [catalogModelsLoading, isBusy, selectedModelId, setSelectedModelId, t, whisperTranscribeModels],
  );
  const webFileInputRef = useRef<HTMLInputElement>(null);

  useHeader({ select: headerModelPicker });

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

  const prepareSelectedModel = async (): Promise<string> => {
    if (!selectedModelId) {
      throw new Error(t('pages.audio.error.selectModelFirst'));
    }

    const model = whisperTranscribeModels.find((item) => item.id === selectedModelId);
    if (!model) {
      throw new Error(t('pages.audio.error.selectedModelMissing'));
    }

    const { downloadedNow } = await ensureDownloadedTranscriptionModel(selectedModelId);

    if (downloadedNow) {
      toast.success(t('pages.audio.toast.downloaded', { model: model.display_name }));
    }

    await loadTranscriptionModel(selectedModelId);

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

      const model = whisperVadModels.find((item) => item.id === selectedVadModelId);
      if (!model) {
        throw new Error(t('pages.audio.error.selectedVadMissing'));
      }
      if (!modelSupportsCapability(model, 'audio_vad')) {
        throw new Error(t('pages.audio.error.selectedModelNotDedicatedVad'));
      }

      const preparedModel = await ensureDownloadedVadModel(selectedVadModelId);
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

      const inferenceOptions = prepareInferenceOptions(controls);
      if (inferenceOptions) {
        transcribeOptions = inferenceOptions;
      }

      if (enableVad) {
        const preparedVad = await prepareVadSettings(refreshedModelConfigDocument);
        transcribeOptions = { ...transcribeOptions, vad: preparedVad.settings };
        vadDescription = t('pages.audio.summary.vadOn', { model: preparedVad.modelName });
      }

      const decodeOptions = prepareDecodeOptions(controls, t);
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

  const refillFromHistory = useCallback((task: AudioTranscriptionTask) => {
    const request = task.request_data;
    const nextModelId = request.model_id ?? selectedModelId;
    setFile({
      file: request.source_path,
      name: fileNameFromPath(request.source_path),
    });
    if (request.model_id) {
      setSelectedModelId(request.model_id);
    }

    if (nextModelId) {
      const vad = request.vad;
      const decode = request.decode;
      setModelControlOverrides(nextModelId, normalizeAudioTranscriptionControls({
        decodeDurationMs: numericString(decode?.duration_ms),
        decodeEntropyThold: numericString(decode?.entropy_thold),
        decodeLogprobThold: numericString(decode?.logprob_thold),
        decodeMaxLen: numericString(decode?.max_len),
        decodeMaxTokens: numericString(decode?.max_tokens),
        decodeNoContext: Boolean(decode?.no_context),
        decodeNoSpeechThold: numericString(decode?.no_speech_thold),
        decodeNoTimestamps: Boolean(decode?.no_timestamps),
        decodeOffsetMs: numericString(decode?.offset_ms),
        decodeSplitOnWord: Boolean(decode?.split_on_word),
        decodeSuppressNst: Boolean(decode?.suppress_nst),
        decodeTdrzEnable: Boolean(decode?.tdrz_enable),
        decodeTemperature: numericString(decode?.temperature),
        decodeTemperatureInc: numericString(decode?.temperature_inc),
        decodeTokenTimestamps: Boolean(decode?.token_timestamps),
        decodeWordThold: numericString(decode?.word_thold),
        detectLanguage: Boolean(request.detect_language) || request.language === 'auto',
        enableVad: Boolean(vad?.enabled),
        language: request.language === 'auto' ? '' : request.language ?? '',
        prompt: request.prompt ?? '',
        selectedVadModelId,
        showDecodeOptions: Boolean(decode),
        vadMaxSpeechDurationS: numericString(vad?.max_speech_duration_s),
        vadMinSilenceDurationMs: numericString(vad?.min_silence_duration_ms),
        vadMinSpeechDurationMs: numericString(vad?.min_speech_duration_ms),
        vadSamplesOverlap: numericString(vad?.samples_overlap),
        vadSpeechPadMs: numericString(vad?.speech_pad_ms),
        vadThreshold: numericString(vad?.threshold),
      }));
    }

    setHistoryDialogOpen(false);
    toast.success(t('pages.audio.history.refilled'));
  }, [
    selectedModelId,
    selectedVadModelId,
    setHistoryDialogOpen,
    setModelControlOverrides,
    setSelectedModelId,
    t,
  ]);

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
    refillFromHistory,
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

function numericString(value: number | null | undefined) {
  return typeof value === 'number' && Number.isFinite(value) ? String(value) : '';
}

function fileNameFromPath(path: string) {
  return path.match(/[^/\\]+$/)?.[0] ?? path;
}
