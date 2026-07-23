import { describe, expect, it } from 'vitest';

import type { AiModel } from '@/hooks/use-ai-model';

import { mergeAudioModels } from '../audio-model-catalog';

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['audio_transcription'],
    chat_capabilities: null,
    created_at: '2026-01-01T00:00:00Z',
    display_name: 'Model',
    filename: 'model.gguf',
    id: 'model-1',
    kind: 'local',
    local_path: null,
    pending: false,
    repo_id: 'owner/model',
    runtime_state: null,
    size_bytes: null,
    spec: {
      filename: 'model.gguf',
      local_path: null,
      provider_id: null,
      remote_model_id: null,
      repo_id: 'owner/model',
    },
    status: 'ready',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('mergeAudioModels', () => {
  it('concatenates disjoint transcription and vad catalogs', () => {
    const result = mergeAudioModels(
      [aiModel({ id: 'whisper-1', display_name: 'Whisper' })],
      [aiModel({ id: 'vad-1', display_name: 'Silero VAD' })],
    );

    expect(result.map((model) => model.id)).toEqual(['whisper-1', 'vad-1']);
  });

  it('dedupes by id, letting the vad entry win and preserving first-seen order', () => {
    const result = mergeAudioModels(
      [
        aiModel({ id: 'shared', display_name: 'Transcription' }),
        aiModel({ id: 'only-transcription' }),
      ],
      [aiModel({ id: 'shared', display_name: 'VAD' }), aiModel({ id: 'only-vad' })],
    );

    expect(result.map((model) => model.id)).toEqual([
      'shared',
      'only-transcription',
      'only-vad',
    ]);
    expect(result[0]?.display_name).toBe('VAD');
  });

  it('returns an empty list when both catalogs are empty', () => {
    expect(mergeAudioModels([], [])).toEqual([]);
  });
});
