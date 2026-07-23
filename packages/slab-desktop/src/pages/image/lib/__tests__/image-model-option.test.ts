import { describe, expect, it } from 'vitest';

import type { AiModel } from '@/hooks/use-ai-model';

import { toImageModelOption } from '../image-model-option';

function aiModel(overrides: Partial<AiModel> = {}): AiModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['image_generation'],
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

describe('toImageModelOption', () => {
  it('maps id, display name and pending state', () => {
    expect(
      toImageModelOption(aiModel({ id: 'sdxl-1', display_name: 'SDXL', pending: true })),
    ).toEqual({
      id: 'sdxl-1',
      label: 'SDXL',
      downloaded: false,
      pending: true,
      local_path: null,
    });
  });

  it('marks models with a local path as downloaded', () => {
    expect(
      toImageModelOption(aiModel({ local_path: '/models/sdxl.gguf' })).downloaded,
    ).toBe(true);
  });

  it('normalizes a missing local path to null and reports not downloaded', () => {
    const option = toImageModelOption(aiModel({ local_path: null }));
    expect(option.downloaded).toBe(false);
    expect(option.local_path).toBeNull();
  });
});
