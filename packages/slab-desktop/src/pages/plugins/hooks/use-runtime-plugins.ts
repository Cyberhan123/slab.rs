import api from '@slab/api';

export const RUNTIME_PLUGINS_QUERY_KEY = ['plugin-runtime-list'] as const;

export function useRuntimePlugins() {
  return api.useQuery('get', '/v1/plugins', undefined, {
    retry: false,
    staleTime: 10_000,
  });
}
