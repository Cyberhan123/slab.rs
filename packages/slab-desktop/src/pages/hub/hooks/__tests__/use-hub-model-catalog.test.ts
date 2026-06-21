import { describe, expect, it } from 'vitest';

import {
  classifyByCapabilities,
  getModelUseRoute,
  type ModelItem,
  toModelItem,
} from '../use-hub-model-catalog';
import type { CatalogModel } from '@slab/api/models';

function model(
  capabilities: ModelItem['capabilities'],
  overrides: Partial<Pick<ModelItem, 'display_name' | 'filename' | 'repo_id'>> = {},
) {
  return {
    backend_ids: [],
    capabilities,
    display_name: overrides.display_name ?? 'Model',
    filename: overrides.filename ?? '',
    kind: 'local' as const,
    repo_id: overrides.repo_id ?? '',
  };
}

describe('hub model catalog helpers', () => {
  it('classifies by capabilities before filename fallback', () => {
    expect(
      classifyByCapabilities(
        model(['audio_transcription'], {
          filename: 'stable-diffusion-image-model.gguf',
        }),
      ),
    ).toBe('audio');
    expect(classifyByCapabilities(model(['image_generation']))).toBe('vision');
    expect(classifyByCapabilities(model(['image_embedding']))).toBe('embedding');
    expect(classifyByCapabilities(model([], { filename: 'coder-model.gguf' }))).toBe('coding');
  });

  it('maps usable categories to product routes and disables embedding', () => {
    expect(getModelUseRoute({ category: 'language' })).toBe('/assistant');
    expect(getModelUseRoute({ category: 'vision' })).toBe('/image');
    expect(getModelUseRoute({ category: 'audio' })).toBe('/audio');
    expect(getModelUseRoute({ category: 'embedding' })).toBeNull();
  });

  it('keeps backend model status authoritative over download tracking', () => {
    const item = toModelItem(
      catalogModel({
        local_path: null,
        status: 'not_downloaded',
      }),
      {
        progress: {
          current: 4,
          label: null,
          message: null,
          step: null,
          stepCount: null,
          total: 10,
          unit: 'bytes',
        },
        taskId: 'task-1',
      },
      null,
    );

    expect(item.status).toBe('not_downloaded');
    expect(item.pending).toBe(false);
    expect(item.download_task_id).toBe('task-1');
    expect(item.download_progress?.current).toBe(4);
  });

  it('marks backend downloading models pending without local tracking', () => {
    const item = toModelItem(
      catalogModel({
        status: 'downloading',
      }),
      undefined,
      null,
    );

    expect(item.status).toBe('downloading');
    expect(item.pending).toBe(true);
    expect(item.download_task_id).toBeNull();
    expect(item.download_progress).toBeNull();
  });
});

function catalogModel(overrides: Partial<CatalogModel> = {}): CatalogModel {
  return {
    backend_id: null,
    backend_ids: [],
    capabilities: ['text_generation'],
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
