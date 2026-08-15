import { describe, expect, it } from 'vitest';

import { createDefaultAudioTranscriptionControls } from '../audio-transcription-controls';
import { prepareDecodeOptions, prepareInferenceOptions } from '../audio-transcription-options';
import type { Translate } from '../audio-value-parsing';

const t: Translate = (key) => key;

describe('prepareInferenceOptions', () => {
  it('returns undefined when language, prompt and auto-detect are all empty', () => {
    expect(
      prepareInferenceOptions({ language: '', prompt: '', detectLanguage: false }),
    ).toBeUndefined();
  });

  it('trims and keeps an explicit language and prompt', () => {
    expect(
      prepareInferenceOptions({ language: '  en  ', prompt: '  hi  ', detectLanguage: false }),
    ).toEqual({ language: 'en', prompt: 'hi' });
  });

  it('forces language to auto when detection is enabled and no language is set', () => {
    expect(
      prepareInferenceOptions({ language: '', prompt: '', detectLanguage: true }),
    ).toEqual({ language: 'auto' });
  });

  it('prefers an explicit language over auto-detection', () => {
    expect(
      prepareInferenceOptions({ language: 'fr', prompt: '', detectLanguage: true }),
    ).toEqual({ language: 'fr' });
  });
});

describe('prepareDecodeOptions', () => {
  it('returns undefined when decode options are hidden', () => {
    const controls = {
      ...createDefaultAudioTranscriptionControls(),
      showDecodeOptions: false,
      decodeOffsetMs: '100',
    };

    expect(prepareDecodeOptions(controls, t)).toBeUndefined();
  });

  it('parses numeric decode fields onto the decode object', () => {
    const controls = {
      ...createDefaultAudioTranscriptionControls(),
      showDecodeOptions: true,
      decodeOffsetMs: '100',
      decodeDurationMs: '5000',
      decodeWordThold: '0.5',
      decodeTemperature: '0.8',
    };

    expect(prepareDecodeOptions(controls, t)).toEqual({
      offset_ms: 100,
      duration_ms: 5000,
      word_thold: 0.5,
      temperature: 0.8,
    });
  });

  it('surfaces boolean decode flags', () => {
    const controls = {
      ...createDefaultAudioTranscriptionControls(),
      showDecodeOptions: true,
      decodeNoContext: true,
      decodeTokenTimestamps: true,
    };

    expect(prepareDecodeOptions(controls, t)).toEqual({
      no_context: true,
      token_timestamps: true,
    });
  });

  it('returns undefined when decode is shown but every field is empty', () => {
    const controls = { ...createDefaultAudioTranscriptionControls(), showDecodeOptions: true };

    expect(prepareDecodeOptions(controls, t)).toBeUndefined();
  });

  it('throws when a decode integer is invalid', () => {
    const controls = {
      ...createDefaultAudioTranscriptionControls(),
      showDecodeOptions: true,
      decodeOffsetMs: 'not-a-number',
    };

    expect(() => prepareDecodeOptions(controls, t)).toThrow('pages.audio.validation.integer');
  });

  it('throws when a float falls outside its allowed range', () => {
    const controls = {
      ...createDefaultAudioTranscriptionControls(),
      showDecodeOptions: true,
      decodeWordThold: '2.5',
    };

    expect(() => prepareDecodeOptions(controls, t)).toThrow('pages.audio.validation.max');
  });
});
