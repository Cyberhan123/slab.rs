import api from '@/lib/api';
import type { components } from '@/lib/api/v1.d.ts';

export type ModelConfigFieldScope = components['schemas']['ModelConfigFieldScopeResponse'];
export type ModelConfigValueType = components['schemas']['ModelConfigValueTypeResponse'];
export type ModelConfigOrigin = components['schemas']['ModelConfigOriginResponse'];
export type ModelConfigPresetOption = components['schemas']['ModelConfigPresetOptionResponse'];
export type ModelConfigVariantOption = components['schemas']['ModelConfigVariantOptionResponse'];
export type ModelConfigSelectionResponse = components['schemas']['ModelConfigSelectionResponse'];
export type ModelConfigSourceArtifact = components['schemas']['ModelConfigSourceArtifactResponse'];
export type ModelConfigSourceSummary = components['schemas']['ModelConfigSourceSummaryResponse'];
export type ModelConfigFieldResponse = components['schemas']['ModelConfigFieldResponse'];
export type ModelConfigSectionResponse = components['schemas']['ModelConfigSectionResponse'];
export type ModelConfigDocumentResponse = components['schemas']['ModelConfigDocumentResponse'];
export type UpdateModelConfigSelectionRequest = components['schemas']['UpdateModelConfigSelectionRequest'];
export type UnifiedModelResponse = components['schemas']['UnifiedModelResponse'];

export function useModelConfigDocumentQuery(id: string | null, options?: { enabled?: boolean }) {
  return api.useQuery(
    'get',
    '/v1/models/{id}/config-document',
    {
      params: {
        path: {
          id: id ?? '',
        },
      },
    },
    {
      enabled: Boolean(id) && (options?.enabled ?? true),
      retry: false,
    },
  ) as {
    data: ModelConfigDocumentResponse | undefined;
    error: unknown;
    isLoading: boolean;
    refetch: () => Promise<{ data: ModelConfigDocumentResponse | undefined }>;
  };
}

export function useUpdateModelConfigSelectionMutation() {
  return api.useMutation('put', '/v1/models/{id}/config-selection') as {
    isPending: boolean;
    mutateAsync: (options: {
      body: UpdateModelConfigSelectionRequest;
      params: {
        path: {
          id: string;
        };
      };
    }) => Promise<UnifiedModelResponse>;
  };
}

export function getModelConfigField(
  document: ModelConfigDocumentResponse,
  path: string,
): ModelConfigFieldResponse | null {
  for (const section of document.sections) {
    const field = section.fields.find((candidate) => candidate.path === path);
    if (field) {
      return field;
    }
  }

  return null;
}

export function getModelConfigFieldValue<T = unknown>(
  document: ModelConfigDocumentResponse,
  path: string,
): T | undefined {
  return getModelConfigField(document, path)?.effective_value as T | undefined;
}
