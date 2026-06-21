import { describe, expect, it } from 'vitest';

import {
  classifyByCapabilities,
  getModelUseRoute,
  type ModelItem,
} from '../use-hub-model-catalog';

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
});
