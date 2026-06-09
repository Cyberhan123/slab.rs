import type { ModelConfigDocumentResponse } from '@/lib/model-config';

export type BundledVadArtifact = {
  id: string;
  label: string;
  value: string;
};

export function findBundledVadArtifact(
  document: ModelConfigDocumentResponse | undefined,
): BundledVadArtifact | null {
  const artifacts = document?.source_summary?.artifacts;
  if (!Array.isArray(artifacts) || artifacts.length === 0) {
    return null;
  }

  const exactMatch = artifacts.find((artifact) => {
    const normalizedId = artifact.id.trim().toLowerCase();
    return normalizedId === 'vad' || normalizedId === 'audio_vad';
  });
  if (exactMatch) {
    return exactMatch;
  }

  const fuzzyMatch = artifacts.find((artifact) => {
    const normalizedId = artifact.id.trim().toLowerCase();
    return (
      normalizedId.endsWith('/vad') ||
      normalizedId.endsWith('_vad') ||
      normalizedId.includes('vad')
    );
  });

  return fuzzyMatch ?? null;
}
