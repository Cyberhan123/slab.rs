import { beforeEach, describe, expect, it, vi } from 'vitest';

import api from '@slab/api';
import type { ModelConfigDocumentResponse } from '../model-config';
import {
  getModelConfigField,
  getModelConfigFieldValue,
  useModelConfigDocumentQuery,
  useUpdateModelConfigSelectionMutation,
} from '../model-config';

vi.mock('@slab/api', () => ({
  default: {
    useMutation: vi.fn<() => unknown>(),
    useQuery: vi.fn<() => unknown>(),
  },
}));

const mockedApi = vi.mocked(api);

describe('model config helpers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('finds fields across sections and returns typed effective values', () => {
    const document = modelConfigDocument();

    expect(getModelConfigField(document, 'source.repo_id')).toMatchObject({
      path: 'source.repo_id',
      effective_value: 'Qwen/Qwen3',
    });
    expect(getModelConfigFieldValue<string>(document, 'source.repo_id')).toBe('Qwen/Qwen3');
    expect(getModelConfigFieldValue<number>(document, 'inference.temperature')).toBe(0.7);
    expect(getModelConfigField(document, 'missing.path')).toBeNull();
    expect(getModelConfigFieldValue(document, 'missing.path')).toBeUndefined();
  });

  it('passes disabled query options when no model id is available', () => {
    mockedApi.useQuery.mockReturnValue({
      data: undefined,
      error: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<{ data: ModelConfigDocumentResponse | undefined }>>(),
    });

    useModelConfigDocumentQuery(null);

    expect(mockedApi.useQuery).toHaveBeenCalledWith(
      'get',
      '/v1/models/{id}/config-document',
      {
        params: {
          path: {
            id: '',
          },
        },
      },
      {
        enabled: false,
        retry: false,
      },
    );
  });

  it('enables config document query only when id and options allow it', () => {
    mockedApi.useQuery.mockReturnValue({
      data: modelConfigDocument(),
      error: null,
      isLoading: false,
      refetch: vi.fn<() => Promise<{ data: ModelConfigDocumentResponse | undefined }>>(),
    });

    useModelConfigDocumentQuery('model-1', { enabled: false });
    useModelConfigDocumentQuery('model-1');

    expect(mockedApi.useQuery).toHaveBeenNthCalledWith(
      1,
      'get',
      '/v1/models/{id}/config-document',
      {
        params: {
          path: {
            id: 'model-1',
          },
        },
      },
      {
        enabled: false,
        retry: false,
      },
    );
    expect(mockedApi.useQuery).toHaveBeenNthCalledWith(
      2,
      'get',
      '/v1/models/{id}/config-document',
      {
        params: {
          path: {
            id: 'model-1',
          },
        },
      },
      {
        enabled: true,
        retry: false,
      },
    );
  });

  it('uses the generated API mutation for config selection updates', () => {
    const mutation = {
      isPending: false,
      mutateAsync: vi.fn<() => Promise<unknown>>(),
    };
    mockedApi.useMutation.mockReturnValue(mutation);

    expect(useUpdateModelConfigSelectionMutation()).toBe(mutation);
    expect(mockedApi.useMutation).toHaveBeenCalledWith(
      'put',
      '/v1/models/{id}/config-selection',
    );
  });
});

function modelConfigDocument(): ModelConfigDocumentResponse {
  return {
    sections: [
      {
        id: 'source',
        label: 'Source',
        fields: [
          {
            path: 'source.repo_id',
            scope: 'source',
            label: 'Repo ID',
            value_type: 'string',
            effective_value: 'Qwen/Qwen3',
            origin: 'pack_manifest',
            editable: false,
            locked: true,
          },
        ],
      },
      {
        id: 'inference',
        label: 'Inference',
        fields: [
          {
            path: 'inference.temperature',
            scope: 'inference',
            label: 'Temperature',
            value_type: 'number',
            effective_value: 0.7,
            origin: 'selected_backend_config',
            editable: false,
            locked: true,
          },
        ],
      },
    ],
  } as ModelConfigDocumentResponse;
}
