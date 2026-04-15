import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ChangeEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import useFile, { type SelectedFile } from '@/hooks/use-file';
import { usePageHeader, usePageHeaderControl } from '@/hooks/use-global-header-meta';
import { usePersistedHeaderSelect } from '@/hooks/use-persisted-header-select';
import useIsTauri from '@/hooks/use-tauri';
import api from '@/lib/api';
import { modelSupportsCapability, toCatalogModelList, type CatalogModel } from '@/lib/api/models';
import {
  useModelConfigDocumentQuery,
  type ModelConfigDocumentResponse,
} from '@/lib/model-config';
import { HEADER_SELECT_KEYS } from '@/layouts/header-controls';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import { useAudioUiStore } from '@/store/useAudioUiStore';
import {
  MODEL_DOWNLOAD_POLL_INTERVAL_MS,
  MODEL_DOWNLOAD_TIMEOUT_MS,
  type PreparingStage,
} from '../const';
import {
  areAudioTranscriptionControlValuesEqual,
  buildAudioTranscriptionControlsFromModelConfig,
  createDefaultAudioTranscriptionControls,
  normalizeAudioTranscriptionControls,
  type AudioTranscriptionControls,
} from '../lib/audio-transcription-controls';
import useTranscribe, { type TranscribeOptions, type TranscribeVadSettings } from './use-transcribe';

const BUNDLED_VAD_MODEL_ID = '__bundled_vad__';

type BundledVadArtifact = {
  id: string;
  label: string;
  value: string;
};

function findBundledVadArtifact(
  document: ModelConfigDocumentResponse | undefined,
): BundledVadArtifact | null {
  const artifacts = document?.source_summary?.artifacts;
  if (!Array.isArray(artifacts) || artifacts.length === 0) {
    return null;
  }

  const exactMatch = artifacts.find((artifact) => {
    const normalizedId = artifact.id.trim().toLowerCase();
    return normalizedId === 'vad' || normalizedId === 'audio_vad';
  });
  if (exactMatch) {
    return exactMatch;
  }

  const fuzzyMatch = artifacts.find((artifact) => {
    const normalizedId = artifact.id.trim().toLowerCase();
    return (
      normalizedId.endsWith('/vad') ||
      normalizedId.endsWith('_vad') ||
      normalizedId.includes('vad')
    );
  });

  return fuzzyMatch ?? null;
}

export function useAudio() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const isTauri = useIsTauri();
  usePageHeader({
    icon: PAGE_HEADER_META.audio.icon,
    title: t('pages.audio.header.title'),
    subtitle: t('pages.audio.header.subtitle'),
  });

  const [file, setFile] = useState<SelectedFile | null>(null);
  const [preparingStage, setPreparingStage] = useState<PreparingStage>(null);
  const [taskId, setTaskId] = useState<string | null>(null);

  const { handleFile } = useFile();
  const transcribe = useTranscribe();
  const hasHydrated = useAudioUiStore((state) => state.hasHydrated);
  const modelControlOverrides = useAudioUiStore((state) => state.modelControlOverrides);
  const setModelControlOverrides = useAudioUiStore((state) => state.setModelControlOverrides);
  const clearModelControlOverrides = useAudioUiStore((state) => state.clearModelControlOverrides);
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
        ...(controlOverrides ?? {}),
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

      const nextOverrides = { ...(controlOverrides ?? {}) };
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
        throw new Error(
          task.error_msg ??
            t('pages.hub.error.taskEndedWithStatus', {
              taskId: tid,
              status: task.status,
            }),
        );
      }

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

  const parseOptionalInt = (raw: string, fieldLabel: string, min: number): number | undefined => {
    const trimmed = raw.trim();
    if (!trimmed) return undefined;

    const parsed = Number(trimmed);
    if (!Number.isInteger(parsed)) {
      throw new Error(t('pages.audio.validation.integer', { label: fieldLabel }));
    }
    if (parsed < min) {
      throw new Error(t('pages.audio.validation.min', { label: fieldLabel, value: min }));
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
      throw new Error(t('pages.audio.validation.number', { label: fieldLabel }));
    }
    if (options.min !== undefined && parsed < options.min) {
      throw new Error(
        t('pages.audio.validation.min', { label: fieldLabel, value: options.min }),
      );
    }
    if (options.max !== undefined && parsed > options.max) {
      throw new Error(
        t('pages.audio.validation.max', { label: fieldLabel, value: options.max }),
      );
    }
    if (options.exclusiveMin !== undefined && parsed <= options.exclusiveMin) {
      throw new Error(
        t('pages.audio.validation.exclusiveMin', {
          label: fieldLabel,
          value: options.exclusiveMin,
        }),
      );
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
      { min: 0, max: 1 },
    );
    const minSpeechDurationMs = parseOptionalInt(
      vadMinSpeechDurationMs,
      t('pages.audio.validation.labels.vadMinSpeechDurationMs'),
      0,
    );
    const minSilenceDurationMs = parseOptionalInt(
      vadMinSilenceDurationMs,
      t('pages.audio.validation.labels.vadMinSilenceDurationMs'),
      0,
    );
    const maxSpeechDurationS = parseOptionalFloat(
      vadMaxSpeechDurationS,
      t('pages.audio.validation.labels.vadMaxSpeechDurationS'),
      {
        exclusiveMin: 0,
      },
    );
    const speechPadMs = parseOptionalInt(
      vadSpeechPadMs,
      t('pages.audio.validation.labels.vadSpeechPadMs'),
      0,
    );
    const samplesOverlap = parseOptionalFloat(
      vadSamplesOverlap,
      t('pages.audio.validation.labels.vadSamplesOverlap'),
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
      next.detect_language = true;
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
    );
    const durationMs = parseOptionalInt(
      decodeDurationMs,
      t('pages.audio.validation.labels.decodeDurationMs'),
      0,
    );
    const wordThold = parseOptionalFloat(
      decodeWordThold,
      t('pages.audio.validation.labels.decodeWordThreshold'),
      { min: 0, max: 1 },
    );
    const maxLen = parseOptionalInt(
      decodeMaxLen,
      t('pages.audio.validation.labels.decodeMaxSegmentLength'),
      0,
    );
    const maxTokens = parseOptionalInt(
      decodeMaxTokens,
      t('pages.audio.validation.labels.decodeMaxTokensPerSegment'),
      0,
    );
    const temperature = parseOptionalFloat(
      decodeTemperature,
      t('pages.audio.validation.labels.decodeTemperature'),
      { min: 0 },
    );
    const temperatureInc = parseOptionalFloat(
      decodeTemperatureInc,
      t('pages.audio.validation.labels.decodeTemperatureIncrement'),
      { min: 0 },
    );
    const entropyThold = parseOptionalFloat(
      decodeEntropyThold,
      t('pages.audio.validation.labels.decodeEntropyThreshold'),
    );
    const logprobThold = parseOptionalFloat(
      decodeLogprobThold,
      t('pages.audio.validation.labels.decodeLogprobThreshold'),
    );
    const noSpeechThold = parseOptionalFloat(
      decodeNoSpeechThold,
      t('pages.audio.validation.labels.decodeNoSpeechThreshold'),
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
      setTaskId(null);
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
        transcribeOptions = { ...(transcribeOptions ?? {}), vad: preparedVad.settings };
        vadDescription = t('pages.audio.summary.vadOn', { model: preparedVad.modelName });
      }

      const decodeOptions = prepareDecodeOptions();
      if (decodeOptions) {
        transcribeOptions = { ...(transcribeOptions ?? {}), decode: decodeOptions };
        decodeDescription = t('pages.audio.summary.decodeCustom');
      }

      setPreparingStage('transcribe');
      const result = await transcribe.handleTranscribe(file.file, transcribeOptions);
      setTaskId(result.operation_id);

      toast.success(t('pages.audio.toast.taskCreated'), {
        description: t('pages.audio.toast.taskCreatedDescription', {
          id: result.operation_id,
          model: modelName,
          vad: vadDescription,
          decode: decodeDescription,
        }),
        action: {
          label: t('pages.audio.toast.viewTasks'),
          onClick: () => navigate('/task'),
        },
      });
    } catch (err: unknown) {
      const anyErr = err as { message?: string; error?: string } | null;
      toast.error(t('pages.audio.toast.failedToCreateTask'), {
        description: anyErr?.message || anyErr?.error || t('pages.audio.toast.unknownError'),
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
    handleFileChange,
    handleTauriFileSelect,
    handleTranscribe,
    hasBundledVad,
    isBusy,
    isTauri,
    isUsingBundledVad,
    navigate,
    preparingStage,
    previewRows,
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
    setSelectedVadModelId: (value: string) => updateControl('selectedVadModelId', value),
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
