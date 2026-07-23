import type { AiModel } from '@/hooks/use-ai-model';

export type ImageModelOption = {
  id: string;
  label: string;
  downloaded: boolean;
  pending: boolean;
  local_path: string | null;
};

/**
 * Maps a catalog model to the image-generation picker option shape. Extracted
 * from useImageModelPreparation for direct unit testing.
 */
export function toImageModelOption(model: AiModel): ImageModelOption {
  return {
    id: model.id,
    label: model.display_name,
    downloaded: Boolean(model.local_path),
    pending: model.pending,
    local_path: model.local_path ?? null,
  };
}
