import { useQuery } from '@tanstack/react-query';

import { isTauri } from '@/hooks/use-tauri';
import { pluginRuntimeList } from '@/lib/plugin-host-bridge';

export const RUNTIME_PLUGINS_QUERY_KEY = ['tauri-plugin-runtime-list'] as const;

export function useRuntimePlugins() {
  return useQuery({
    queryKey: RUNTIME_PLUGINS_QUERY_KEY,
    queryFn: pluginRuntimeList,
    enabled: isTauri(),
    retry: false,
    staleTime: 10_000,
  });
}
