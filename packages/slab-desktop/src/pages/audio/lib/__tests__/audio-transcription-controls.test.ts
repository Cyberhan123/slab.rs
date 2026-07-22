import { describe, expect, it } from 'vitest';

import type { ModelConfigDocumentResponse } from '@/lib/model-config';
import {
  areAudioTranscriptionControlValuesEqual,
  buildAudioTranscriptionControlsFromModelConfig,
  createDefaultAudioTranscriptionControls,
  normalizeAudioTranscriptionControls,
} from '../audio-transcription-controls';

describe('audio transcription controls', () => {
  it('normalizes invalid persisted values back to safe defaults', () => {
    expect(
      normalizeAudioTranscriptionControls({
        enableVad: 'yes',
        vadThreshold: 0.5,
        vadMinSpeechDurationMs: '2000',
        showDecodeOptions: 'true',
        decodeNoContext: 1,
        decodeTemperature: '0.1',
        language: 'en',
        detectLanguage: 'yes',
      }),
    ).toEqual({
      ...createDefaultAudioTranscriptionControls(),
      vadMinSpeechDurationMs: '2000',
      decodeTemperature: '0.1',
      language: 'en',
    });

    // Null and omitted inputs collapse to a fresh default snapshot.
    expect(normalizeAudioTranscriptionControls(null)).toEqual(
      createDefaultAudioTranscriptionControls(),
    );
    expect(normalizeAudioTranscriptionControls()).toEqual(
      createDefaultAudioTranscriptionControls(),
    );
  });

  it('keeps valid persisted controls and compares field values after normalization', () => {
    const controls = normalizeAudioTranscriptionControls({
      enableVad: false,
      vadThreshold: '0.5',
      vadMinSpeechDurationMs: '2000',
      showDecodeOptions: true,
      decodeTemperature: '0.1',
      decodeNoContext: true,
      language: 'en',
      detectLanguage: true,
    });

    expect(controls).toMatchObject({
      enableVad: false,
      vadThreshold: '0.5',
      vadMinSpeechDurationMs: '2000',
      showDecodeOptions: true,
      decodeTemperature: '0.1',
      decodeNoContext: true,
      language: 'en',
      detectLanguage: true,
    });

    // The comparator works on individual field values via Object.is.
    expect(areAudioTranscriptionControlValuesEqual(controls.vadThreshold, '0.5')).toBe(true);
    expect(areAudioTranscriptionControlValuesEqual(controls.vadThreshold, '0.6')).toBe(false);
    expect(areAudioTranscriptionControlValuesEqual(controls.enableVad, false)).toBe(true);
    // Object.is distinguishes NaN and signed zero unlike ===.
    expect(areAudioTranscriptionControlValuesEqual(Number.NaN, Number.NaN)).toBe(true);
    expect(areAudioTranscriptionControlValuesEqual(0, -0)).toBe(false);
  });

  it('builds controls from resolved model config and falls back on invalid specs', () => {
    expect(
      buildAudioTranscriptionControlsFromModelConfig(
        modelConfigDocument({
          vad: {
            enabled: true,
            threshold: 0.5,
            min_speech_duration_ms: 2000,
            min_silence_duration_ms: 500,
            max_speech_duration_s: 30,
            speech_pad_ms: 400,
            samples_overlap: 0.1,
          },
          decode: {
            offset_ms: 0,
            no_context: true,
            temperature: 0.1,
            token_timestamps: true,
          },
          language: 'en',
          detect_language: false,
          prompt: 'transcribe this',
        }),
      ),
    ).toMatchObject({
      enableVad: true,
      vadThreshold: '0.5',
      vadMinSpeechDurationMs: '2000',
      showDecodeOptions: true,
      decodeOffsetMs: '0',
      decodeNoContext: true,
      decodeTemperature: '0.1',
      decodeTokenTimestamps: true,
      language: 'en',
      detectLanguage: false,
      prompt: 'transcribe this',
    });

    // Non-numeric / non-boolean spec values fall back to defaults.
    expect(
      buildAudioTranscriptionControlsFromModelConfig(
        modelConfigDocument({
          vad: { enabled: 'yes', threshold: 'high' },
          decode: {},
          language: 123,
          detect_language: 'maybe',
        }),
      ),
    ).toMatchObject({
      // enableVad defaults to true, so a non-boolean spec falls back to true.
      enableVad: true,
      vadThreshold: '',
      showDecodeOptions: false,
      language: '',
      detectLanguage: false,
    });

    // A configured language of "auto" forces detect-language on and clears the field.
    expect(
      buildAudioTranscriptionControlsFromModelConfig(modelConfigDocument({ language: 'auto' })),
    ).toMatchObject({
      language: '',
      detectLanguage: true,
    });
  });

  it('uses the advanced resolved inference spec field when the top-level spec is absent', () => {
    expect(
      buildAudioTranscriptionControlsFromModelConfig({
        ...modelConfigDocument(null),
        sections: [
          {
            id: 'advanced',
            label: 'Advanced',
            fields: [
              {
                path: 'advanced.resolved_inference_spec',
                scope: 'advanced',
                label: 'Resolved inference spec',
                value_type: 'json',
                effective_value: {
                  vad: { enabled: true, threshold: 0.3 },
                  decode: { temperature: 0.2 },
                  language: 'fr',
                },
                origin: 'derived',
                editable: false,
                locked: true,
              },
            ],
          },
        ],
      }),
    ).toMatchObject({
      enableVad: true,
      vadThreshold: '0.3',
      decodeTemperature: '0.2',
      language: 'fr',
    });
  });
});

function modelConfigDocument(resolved_inference_spec: unknown): ModelConfigDocumentResponse {
  return {
    model_summary: {
      capabilities: ['audio_transcription'],
      created_at: '2026-01-01T00:00:00Z',
      display_name: 'Test Audio Model',
      id: 'test-audio-model',
      kind: 'local',
      spec: {},
      status: 'ready',
      updated_at: '2026-01-01T00:00:00Z',
    },
    resolved_inference_spec,
    resolved_load_spec: {},
    sections: [],
    selection: {
      presets: [],
      variants: [],
    },
    source_summary: {
      artifacts: [],
      source_kind: 'test',
    },
    warnings: [],
  };
}
