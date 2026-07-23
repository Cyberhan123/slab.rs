import type { AiModel } from '@/hooks/use-ai-model';

/**
 * Merges the Whisper transcription and VAD model catalogs, de-duplicating by
 * model id while preserving first-seen insertion order. On an id collision the
 * VAD entry wins (its loop runs second and overwrites). Extracted from
 * useAudioModelCatalog for direct unit testing.
 */
export function mergeAudioModels(transcription: AiModel[], vad: AiModel[]): AiModel[] {
  const merged = new Map<string, AiModel>();
  transcription.forEach((model) => {
    merged.set(model.id, model);
  });
  vad.forEach((model) => {
    merged.set(model.id, model);
  });
  return Array.from(merged.values());
}
