import {
  getModelConfigFieldValue,
  type ModelConfigDocumentResponse,
} from '@/lib/model-config';

type UnknownRecord = Record<string, unknown>;

export type AudioTranscriptionControls = {
  enableVad: boolean;
  selectedVadModelId: string;
  vadThreshold: string;
  vadMinSpeechDurationMs: string;
  vadMinSilenceDurationMs: string;
  vadMaxSpeechDurationS: string;
  vadSpeechPadMs: string;
  vadSamplesOverlap: string;
  showDecodeOptions: boolean;
  decodeOffsetMs: string;
  decodeDurationMs: string;
  decodeWordThold: string;
  decodeMaxLen: string;
  decodeMaxTokens: string;
  decodeTemperature: string;
  decodeTemperatureInc: string;
  decodeEntropyThold: string;
  decodeLogprobThold: string;
  decodeNoSpeechThold: string;
  decodeNoContext: boolean;
  decodeNoTimestamps: boolean;
  decodeTokenTimestamps: boolean;
  decodeSplitOnWord: boolean;
  decodeSuppressNst: boolean;
  decodeTdrzEnable: boolean;
  language: string;
  prompt: string;
  detectLanguage: boolean;
};

const DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS: AudioTranscriptionControls = {
  enableVad: true,
  selectedVadModelId: '',
  vadThreshold: '',
  vadMinSpeechDurationMs: '',
  vadMinSilenceDurationMs: '',
  vadMaxSpeechDurationS: '',
  vadSpeechPadMs: '',
  vadSamplesOverlap: '',
  showDecodeOptions: false,
  decodeOffsetMs: '',
  decodeDurationMs: '',
  decodeWordThold: '',
  decodeMaxLen: '',
  decodeMaxTokens: '',
  decodeTemperature: '',
  decodeTemperatureInc: '',
  decodeEntropyThold: '',
  decodeLogprobThold: '',
  decodeNoSpeechThold: '',
  decodeNoContext: false,
  decodeNoTimestamps: false,
  decodeTokenTimestamps: false,
  decodeSplitOnWord: false,
  decodeSuppressNst: false,
  decodeTdrzEnable: false,
  language: '',
  prompt: '',
  detectLanguage: false,
};

const KNOWN_DECODE_KEYS = [
  'offset_ms',
  'duration_ms',
  'no_context',
  'no_timestamps',
  'token_timestamps',
  'split_on_word',
  'suppress_nst',
  'word_thold',
  'max_len',
  'max_tokens',
  'temperature',
  'temperature_inc',
  'entropy_thold',
  'logprob_thold',
  'no_speech_thold',
  'tdrz_enable',
] as const;

export function createDefaultAudioTranscriptionControls(): AudioTranscriptionControls {
  return { ...DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS };
}

export function normalizeAudioTranscriptionControls(
  value?: Partial<AudioTranscriptionControls> | null,
): AudioTranscriptionControls {
  const next = value ?? {};

  return {
    enableVad:
      typeof next.enableVad === 'boolean'
        ? next.enableVad
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.enableVad,
    selectedVadModelId: normalizeString(
      next.selectedVadModelId,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.selectedVadModelId,
    ),
    vadThreshold: normalizeString(next.vadThreshold, DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadThreshold),
    vadMinSpeechDurationMs: normalizeString(
      next.vadMinSpeechDurationMs,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadMinSpeechDurationMs,
    ),
    vadMinSilenceDurationMs: normalizeString(
      next.vadMinSilenceDurationMs,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadMinSilenceDurationMs,
    ),
    vadMaxSpeechDurationS: normalizeString(
      next.vadMaxSpeechDurationS,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadMaxSpeechDurationS,
    ),
    vadSpeechPadMs: normalizeString(
      next.vadSpeechPadMs,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadSpeechPadMs,
    ),
    vadSamplesOverlap: normalizeString(
      next.vadSamplesOverlap,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.vadSamplesOverlap,
    ),
    showDecodeOptions:
      typeof next.showDecodeOptions === 'boolean'
        ? next.showDecodeOptions
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.showDecodeOptions,
    decodeOffsetMs: normalizeString(
      next.decodeOffsetMs,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeOffsetMs,
    ),
    decodeDurationMs: normalizeString(
      next.decodeDurationMs,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeDurationMs,
    ),
    decodeWordThold: normalizeString(
      next.decodeWordThold,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeWordThold,
    ),
    decodeMaxLen: normalizeString(next.decodeMaxLen, DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeMaxLen),
    decodeMaxTokens: normalizeString(
      next.decodeMaxTokens,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeMaxTokens,
    ),
    decodeTemperature: normalizeString(
      next.decodeTemperature,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeTemperature,
    ),
    decodeTemperatureInc: normalizeString(
      next.decodeTemperatureInc,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeTemperatureInc,
    ),
    decodeEntropyThold: normalizeString(
      next.decodeEntropyThold,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeEntropyThold,
    ),
    decodeLogprobThold: normalizeString(
      next.decodeLogprobThold,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeLogprobThold,
    ),
    decodeNoSpeechThold: normalizeString(
      next.decodeNoSpeechThold,
      DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeNoSpeechThold,
    ),
    decodeNoContext:
      typeof next.decodeNoContext === 'boolean'
        ? next.decodeNoContext
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeNoContext,
    decodeNoTimestamps:
      typeof next.decodeNoTimestamps === 'boolean'
        ? next.decodeNoTimestamps
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeNoTimestamps,
    decodeTokenTimestamps:
      typeof next.decodeTokenTimestamps === 'boolean'
        ? next.decodeTokenTimestamps
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeTokenTimestamps,
    decodeSplitOnWord:
      typeof next.decodeSplitOnWord === 'boolean'
        ? next.decodeSplitOnWord
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeSplitOnWord,
    decodeSuppressNst:
      typeof next.decodeSuppressNst === 'boolean'
        ? next.decodeSuppressNst
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeSuppressNst,
    decodeTdrzEnable:
      typeof next.decodeTdrzEnable === 'boolean'
        ? next.decodeTdrzEnable
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.decodeTdrzEnable,
    language: normalizeString(next.language, DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.language),
    prompt: normalizeString(next.prompt, DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.prompt),
    detectLanguage:
      typeof next.detectLanguage === 'boolean'
        ? next.detectLanguage
        : DEFAULT_AUDIO_TRANSCRIPTION_CONTROLS.detectLanguage,
  };
}

export function buildAudioTranscriptionControlsFromModelConfig(
  document: ModelConfigDocumentResponse,
): AudioTranscriptionControls {
  const inferenceSpec = extractResolvedInferenceSpec(document);
  const vad = isRecord(inferenceSpec.vad) ? inferenceSpec.vad : {};
  const decode = isRecord(inferenceSpec.decode) ? inferenceSpec.decode : {};
  const configuredLanguage = normalizeString(toStringValue(inferenceSpec.language), '');
  const detectLanguage =
    configuredLanguage.trim().toLowerCase() === 'auto'
      ? true
      : toBoolean(inferenceSpec.detect_language);

  return normalizeAudioTranscriptionControls({
    enableVad: toBoolean(vad.enabled),
    vadThreshold: toNumericString(vad.threshold),
    vadMinSpeechDurationMs: toNumericString(vad.min_speech_duration_ms),
    vadMinSilenceDurationMs: toNumericString(vad.min_silence_duration_ms),
    vadMaxSpeechDurationS: toNumericString(vad.max_speech_duration_s),
    vadSpeechPadMs: toNumericString(vad.speech_pad_ms),
    vadSamplesOverlap: toNumericString(vad.samples_overlap),
    showDecodeOptions: hasAnyKnownKey(decode, KNOWN_DECODE_KEYS),
    decodeOffsetMs: toNumericString(decode.offset_ms),
    decodeDurationMs: toNumericString(decode.duration_ms),
    decodeWordThold: toNumericString(decode.word_thold),
    decodeMaxLen: toNumericString(decode.max_len),
    decodeMaxTokens: toNumericString(decode.max_tokens),
    decodeTemperature: toNumericString(decode.temperature),
    decodeTemperatureInc: toNumericString(decode.temperature_inc),
    decodeEntropyThold: toNumericString(decode.entropy_thold),
    decodeLogprobThold: toNumericString(decode.logprob_thold),
    decodeNoSpeechThold: toNumericString(decode.no_speech_thold),
    decodeNoContext: toBoolean(decode.no_context),
    decodeNoTimestamps: toBoolean(decode.no_timestamps),
    decodeTokenTimestamps: toBoolean(decode.token_timestamps),
    decodeSplitOnWord: toBoolean(decode.split_on_word),
    decodeSuppressNst: toBoolean(decode.suppress_nst),
    decodeTdrzEnable: toBoolean(decode.tdrz_enable),
    language: detectLanguage ? '' : configuredLanguage,
    prompt: toStringValue(inferenceSpec.prompt),
    detectLanguage,
  });
}

export function areAudioTranscriptionControlValuesEqual(
  left: AudioTranscriptionControls[keyof AudioTranscriptionControls],
  right: AudioTranscriptionControls[keyof AudioTranscriptionControls],
): boolean {
  return Object.is(left, right);
}

function extractResolvedInferenceSpec(document: ModelConfigDocumentResponse): UnknownRecord {
  if (isRecord(document.resolved_inference_spec)) {
    return document.resolved_inference_spec;
  }

  const fallback = getModelConfigFieldValue(document, 'advanced.resolved_inference_spec');
  return isRecord(fallback) ? fallback : {};
}

function normalizeString(value: string | undefined, fallback: string): string {
  return typeof value === 'string' ? value : fallback;
}

function toBoolean(value: unknown): boolean | undefined {
  return typeof value === 'boolean' ? value : undefined;
}

function toStringValue(value: unknown): string | undefined {
  return typeof value === 'string' ? value : undefined;
}

function toNumericString(value: unknown): string | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? String(value) : undefined;
}

function hasAnyKnownKey(record: UnknownRecord, keys: readonly string[]): boolean {
  return keys.some((key) => record[key] !== undefined && record[key] !== null);
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
