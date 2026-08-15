import { describe, expect, it } from 'vitest';

import type { ModelConfigDocumentResponse } from '@slab/core/models/config';
import { findBundledVadArtifact, type BundledVadArtifact } from '../audio-vad-models';

function artifact(id: string): BundledVadArtifact {
  return { id, label: id, value: `/models/${id}.bin` };
}

function docWithArtifacts(artifacts: unknown): ModelConfigDocumentResponse {
  return { source_summary: { artifacts: artifacts as never } } as never;
}

describe('findBundledVadArtifact', () => {
  it('returns null when there are no usable artifacts', () => {
    expect(findBundledVadArtifact(undefined)).toBeNull();
    expect(findBundledVadArtifact({} as ModelConfigDocumentResponse)).toBeNull();
    expect(findBundledVadArtifact(docWithArtifacts([]))).toBeNull();
    expect(findBundledVadArtifact(docWithArtifacts({}))).toBeNull();
    expect(findBundledVadArtifact(docWithArtifacts(null))).toBeNull();
  });

  it('matches the vad artifact by exact id, case- and whitespace-insensitive', () => {
    expect(findBundledVadArtifact(docWithArtifacts([artifact('vad')]))).toEqual(artifact('vad'));
    expect(findBundledVadArtifact(docWithArtifacts([artifact('audio_vad')]))).toEqual(
      artifact('audio_vad'),
    );
    expect(findBundledVadArtifact(docWithArtifacts([artifact(' VAD ')]))).toEqual(
      artifact(' VAD '),
    );
  });

  it('falls back to a fuzzy id match when no exact id is present', () => {
    expect(findBundledVadArtifact(docWithArtifacts([artifact('models/vad')]))).toEqual(
      artifact('models/vad'),
    );
    expect(findBundledVadArtifact(docWithArtifacts([artifact('silero_vad')]))).toEqual(
      artifact('silero_vad'),
    );
    expect(findBundledVadArtifact(docWithArtifacts([artifact('myvadmodel')]))).toEqual(
      artifact('myvadmodel'),
    );
  });

  it('prefers an exact match over a fuzzy match in the same list', () => {
    expect(
      findBundledVadArtifact(docWithArtifacts([artifact('silero_vad'), artifact('vad')])),
    ).toEqual(artifact('vad'));
  });

  it('returns null when no artifact id mentions vad', () => {
    expect(
      findBundledVadArtifact(docWithArtifacts([artifact('whisper'), artifact('encoder')])),
    ).toBeNull();
  });
});
