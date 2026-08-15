import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@slab/api', () => ({
  default: {
    useQuery: vi.fn<() => unknown>(),
  },
}));

import api from '@slab/api';
import { RUNTIME_PLUGINS_QUERY_KEY, useRuntimePlugins } from '../use-runtime-plugins';

const mockedApi = vi.mocked(api);

describe('useRuntimePlugins', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('queries the runtime plugins list with a short stale time', () => {
    mockedApi.useQuery.mockReturnValue({ data: undefined });

    useRuntimePlugins();

    expect(mockedApi.useQuery).toHaveBeenCalledWith('get', '/v1/plugins', undefined, {
      staleTime: 10_000,
    });
  });

  it('exposes a stable query key for cache invalidation', () => {
    expect(RUNTIME_PLUGINS_QUERY_KEY).toEqual(['plugin-runtime-list']);
  });
});
