import { useQuery } from '@tanstack/react-query';

import { isTauri } from '@/hooks/use-tauri';
import { pluginRuntimeList } from '@/lib/plugin-host-bridge';

export function useRuntimePlugins() {
  return useQuery({
    queryKey: ['tauri-plugin-runtime-list'],
    queryFn: pluginRuntimeList,
    enabled: isTauri(),
    retry: false,
    staleTime: 10_000,
  });
}
