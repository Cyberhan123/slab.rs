import { describe, expect, it } from 'vitest';

import type { ModelConfigDocumentResponse } from '@/lib/model-config';
import { DEFAULT_GENERATION_SIZE } from '../../const';
import {
  areImageGenerationControlsEqual,
  buildImageGenerationControlsFromModelConfig,
  createDefaultImageGenerationControls,
  normalizeImageGenerationControls,
} from '../image-generation-controls';

describe('image generation controls', () => {
  it('normalizes invalid persisted values back to safe defaults', () => {
    expect(
      normalizeImageGenerationControls({
        mode: 'img2img',
        widthStr: '512px',
        heightStr: '4096',
        numImages: 3,
        advancedOpen: true,
        cfgScale: Number.NaN,
        guidance: -1,
        steps: 0,
        seed: 123,
        sampleMethod: 'unknown',
        scheduler: 'unknown',
        clipSkip: -1,
        eta: -0.1,
        strength: 2,
      }),
    ).toEqual({
      ...createDefaultImageGenerationControls(),
      mode: 'img2img',
      advancedOpen: true,
      seed: 123,
    });
  });

  it('keeps valid persisted controls and compares after normalization', () => {
    const controls = normalizeImageGenerationControls({
      widthStr: ' 768 ',
      heightStr: '576',
      numImages: 4,
      cfgScale: 8.5,
      guidance: 4,
      steps: 30,
      sampleMethod: 'euler',
      scheduler: 'karras',
      clipSkip: 2,
      eta: 0.25,
      strength: 0.5,
    });

    expect(controls).toMatchObject({
      widthStr: '768',
      heightStr: '576',
      numImages: 4,
      cfgScale: 8.5,
      guidance: 4,
      steps: 30,
      sampleMethod: 'euler',
      scheduler: 'karras',
      clipSkip: 2,
      eta: 0.25,
      strength: 0.5,
    });
    expect(areImageGenerationControlsEqual(controls, { ...controls, widthStr: '768' })).toBe(true);
    expect(areImageGenerationControlsEqual(controls, { ...controls, widthStr: '1024' })).toBe(
      false,
    );
  });

  it('builds controls from resolved model config and falls back on invalid specs', () => {
    expect(
      buildImageGenerationControlsFromModelConfig(
        modelConfigDocument({
          mode: 'img2img',
          width: 768,
          height: 576,
          n: 2,
          cfg_scale: 9,
          guidance: 5,
          steps: 25,
          seed: 42,
          sample_method: 'heun',
          scheduler: 'ays',
          clip_skip: 1,
          eta: 0.1,
          strength: 0.4,
        }),
      ),
    ).toMatchObject({
      mode: 'img2img',
      widthStr: '768',
      heightStr: '576',
      numImages: 2,
      cfgScale: 9,
      guidance: 5,
      steps: 25,
      seed: 42,
      sampleMethod: 'heun',
      scheduler: 'ays',
      clipSkip: 1,
      eta: 0.1,
      strength: 0.4,
    });

    expect(
      buildImageGenerationControlsFromModelConfig(
        modelConfigDocument({
          width: '768',
          height: 16,
          n: 3,
          strength: 1.5,
        }),
      ),
    ).toMatchObject({
      widthStr: String(DEFAULT_GENERATION_SIZE),
      heightStr: String(DEFAULT_GENERATION_SIZE),
      numImages: 1,
      strength: 0.75,
    });
  });

  it('uses the advanced resolved inference spec field when the top-level spec is absent', () => {
    expect(
      buildImageGenerationControlsFromModelConfig({
        sections: [
          {
            fields: [
              {
                path: 'advanced.resolved_inference_spec',
                effective_value: {
                  width: 1024,
                  height: 768,
                  sample_method: 'lcm',
                },
              },
            ],
          },
        ],
      } as ModelConfigDocumentResponse),
    ).toMatchObject({
      widthStr: '1024',
      heightStr: '768',
      sampleMethod: 'lcm',
    });
  });
});

function modelConfigDocument(resolved_inference_spec: unknown): ModelConfigDocumentResponse {
  return {
    resolved_inference_spec,
    sections: [],
  } as ModelConfigDocumentResponse;
}
