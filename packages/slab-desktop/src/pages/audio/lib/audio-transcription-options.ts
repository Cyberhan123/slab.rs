import type { TranscribeOptions } from '../hooks/use-transcribe';
import type { AudioTranscriptionControls } from './audio-transcription-controls';
import { parseOptionalFloat, parseOptionalInt, type Translate } from './audio-value-parsing';

/**
 * Builds the inference (language/prompt/auto-detect) slice of the transcription
 * request from the audio controls. Pure transform; extracted from useAudio for
 * direct unit testing.
 */
export function prepareInferenceOptions(
  controls: Pick<AudioTranscriptionControls, 'language' | 'prompt' | 'detectLanguage'>,
): Omit<TranscribeOptions, 'decode' | 'vad'> | undefined {
  const next: Omit<TranscribeOptions, 'decode' | 'vad'> = {};
  const trimmedLanguage = controls.language.trim();
  const trimmedPrompt = controls.prompt.trim();

  if (trimmedLanguage) {
    next.language = trimmedLanguage;
  }
  if (trimmedPrompt) {
    next.prompt = trimmedPrompt;
  }
  if (!trimmedLanguage && controls.detectLanguage) {
    next.language = 'auto';
  }

  return Object.keys(next).length > 0 ? next : undefined;
}

/**
 * Builds the decode options slice from the audio controls. Returns undefined
 * when decode controls are hidden or every field is empty. Throws via the
 * parsing helpers on invalid numeric input. Extracted from useAudio.
 */
export function prepareDecodeOptions(
  controls: AudioTranscriptionControls,
  t: Translate,
): TranscribeOptions['decode'] | undefined {
  if (!controls.showDecodeOptions) {
    return undefined;
  }

  const decode: NonNullable<TranscribeOptions['decode']> = {};

  const offsetMs = parseOptionalInt(
    controls.decodeOffsetMs,
    t('pages.audio.validation.labels.decodeOffsetMs'),
    0,
    t,
  );
  const durationMs = parseOptionalInt(
    controls.decodeDurationMs,
    t('pages.audio.validation.labels.decodeDurationMs'),
    0,
    t,
  );
  const wordThold = parseOptionalFloat(
    controls.decodeWordThold,
    t('pages.audio.validation.labels.decodeWordThreshold'),
    t,
    { min: 0, max: 1 },
  );
  const maxLen = parseOptionalInt(
    controls.decodeMaxLen,
    t('pages.audio.validation.labels.decodeMaxSegmentLength'),
    0,
    t,
  );
  const maxTokens = parseOptionalInt(
    controls.decodeMaxTokens,
    t('pages.audio.validation.labels.decodeMaxTokensPerSegment'),
    0,
    t,
  );
  const temperature = parseOptionalFloat(
    controls.decodeTemperature,
    t('pages.audio.validation.labels.decodeTemperature'),
    t,
    { min: 0 },
  );
  const temperatureInc = parseOptionalFloat(
    controls.decodeTemperatureInc,
    t('pages.audio.validation.labels.decodeTemperatureIncrement'),
    t,
    { min: 0 },
  );
  const entropyThold = parseOptionalFloat(
    controls.decodeEntropyThold,
    t('pages.audio.validation.labels.decodeEntropyThreshold'),
    t,
  );
  const logprobThold = parseOptionalFloat(
    controls.decodeLogprobThold,
    t('pages.audio.validation.labels.decodeLogprobThreshold'),
    t,
  );
  const noSpeechThold = parseOptionalFloat(
    controls.decodeNoSpeechThold,
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
  if (controls.decodeNoContext) decode.no_context = true;
  if (controls.decodeNoTimestamps) decode.no_timestamps = true;
  if (controls.decodeTokenTimestamps) decode.token_timestamps = true;
  if (controls.decodeSplitOnWord) decode.split_on_word = true;
  if (controls.decodeSuppressNst) decode.suppress_nst = true;
  if (controls.decodeTdrzEnable) decode.tdrz_enable = true;

  return Object.keys(decode).length > 0 ? decode : undefined;
}
