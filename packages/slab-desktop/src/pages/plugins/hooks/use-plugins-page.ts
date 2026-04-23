import { useCallback, useDeferredValue, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePageHeader, usePageHeaderSearch } from '@/hooks/use-global-header-meta';
import { isTauri } from '@/hooks/use-tauri';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api, { getErrorMessage } from '@/lib/api';
import {
  isPluginRunning,
  marketPluginSearchText,
  pluginSearchText,
  type PluginMarketRecord,
  type PluginRecord,
} from '../utils';

export function usePluginsPage() {
  const { t } = useTranslation();
  const isDesktopTauri = isTauri();

  usePageHeader({
    icon: PAGE_HEADER_META.plugins.icon,
    title: t('pages.plugins.header.title'),
    subtitle: t('pages.plugins.header.subtitle'),
  });

  const [busyPluginId, setBusyPluginId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');

  const headerSearch = useMemo(
    () =>
      isDesktopTauri
        ? {
            type: 'search' as const,
            value: searchQuery,
            onValueChange: setSearchQuery,
            placeholder: t('pages.plugins.search.placeholder'),
            ariaLabel: t('pages.plugins.search.ariaLabel'),
          }
        : null,
    [isDesktopTauri, searchQuery, t],
  );

  usePageHeaderSearch(headerSearch);

  const {
    data: pluginRows,
    error: pluginsError,
    isLoading: pluginsLoading,
    isFetching: pluginsFetching,
    refetch: refetchPlugins,
  } = api.useQuery('get', '/v1/plugins', undefined, {
    enabled: isDesktopTauri,
    retry: false,
  });
  const {
    data: marketRows,
    error: marketPluginsError,
    isLoading: marketPluginsLoading,
    isFetching: marketPluginsFetching,
    refetch: refetchMarketPlugins,
  } = api.useQuery('get', '/v1/plugins/market', undefined, {
    enabled: isDesktopTauri,
    retry: false,
  });

  const installPluginMutation = api.useMutation('post', '/v1/plugins/install');
  const enablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/enable');
  const disablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/disable');
  const startPluginMutation = api.useMutation('post', '/v1/plugins/{id}/start');
  const stopPluginMutation = api.useMutation('post', '/v1/plugins/{id}/stop');

  const plugins = useMemo(() => pluginRows ?? [], [pluginRows]);
  const marketPlugins = useMemo(() => marketRows ?? [], [marketRows]);
  const loading = pluginsLoading || marketPluginsLoading;
  const refreshing = pluginsFetching || marketPluginsFetching;
  const dataError = pluginsError ?? marketPluginsError;

  const deferredSearchQuery = useDeferredValue(searchQuery);
  const normalizedSearchQuery = deferredSearchQuery.trim().toLowerCase();
  const hasSearchQuery = normalizedSearchQuery.length > 0;

  const filteredPlugins = useMemo(() => {
    if (!normalizedSearchQuery) return plugins;
    return plugins.filter((plugin) => pluginSearchText(plugin).includes(normalizedSearchQuery));
  }, [normalizedSearchQuery, plugins]);

  const filteredMarketPlugins = useMemo(() => {
    if (!normalizedSearchQuery) return marketPlugins;
    return marketPlugins.filter((plugin) => marketPluginSearchText(plugin).includes(normalizedSearchQuery));
  }, [marketPlugins, normalizedSearchQuery]);

  const refreshData = useCallback(async () => {
    if (!isDesktopTauri) return;

    const [pluginResult, marketResult] = await Promise.all([refetchPlugins(), refetchMarketPlugins()]);
    const error = pluginResult.error ?? marketResult.error;

    if (error) {
      toast.error(t('pages.plugins.toast.loadFailed'), {
        description: getErrorMessage(error),
      });
    }
  }, [isDesktopTauri, refetchMarketPlugins, refetchPlugins, t]);

  const runAction = useCallback(
    async (pluginId: string, errorTitle: string, action: () => Promise<void>) => {
      setBusyPluginId(pluginId);
      try {
        await action();
      } catch (error) {
        toast.error(errorTitle, {
          description: getErrorMessage(error),
        });
      } finally {
        setBusyPluginId(null);
      }
    },
    [],
  );

  const handlePrimaryAction = useCallback(
    async (plugin: PluginRecord) => {
      if (!plugin.valid) {
        toast.error(t('pages.plugins.toast.invalidPlugin'), {
          description: plugin.error || t('pages.plugins.toast.unknownValidationError'),
        });
        return;
      }

      await runAction(plugin.id, t('pages.plugins.toast.actionFailed', { name: plugin.name }), async () => {
        if (isPluginRunning(plugin)) {
          await stopPluginMutation.mutateAsync({
            params: {
              path: { id: plugin.id },
            },
            body: { lastError: null },
          });
          toast.success(t('pages.plugins.toast.stopped', { name: plugin.name }));
        } else if (!plugin.enabled) {
          await enablePluginMutation.mutateAsync({
            params: {
              path: { id: plugin.id },
            },
          });
          toast.success(t('pages.plugins.toast.enabled', { name: plugin.name }));
        } else {
          await startPluginMutation.mutateAsync({
            params: {
              path: { id: plugin.id },
            },
          });
          toast.success(t('pages.plugins.toast.launched', { name: plugin.name }));
        }

        await refreshData();
      });
    },
    [enablePluginMutation, refreshData, runAction, startPluginMutation, stopPluginMutation, t],
  );

  const handleToggleEnabled = useCallback(
    async (plugin: PluginRecord) => {
      await runAction(plugin.id, t('pages.plugins.toast.actionFailed', { name: plugin.name }), async () => {
        if (plugin.enabled) {
          if (isPluginRunning(plugin)) {
            await stopPluginMutation.mutateAsync({
              params: {
                path: { id: plugin.id },
              },
              body: { lastError: null },
            });
          }

          await disablePluginMutation.mutateAsync({
            params: {
              path: { id: plugin.id },
            },
          });
          toast.success(t('pages.plugins.toast.disabled', { name: plugin.name }));
        } else {
          await enablePluginMutation.mutateAsync({
            params: {
              path: { id: plugin.id },
            },
          });
          toast.success(t('pages.plugins.toast.enabled', { name: plugin.name }));
        }

        await refreshData();
      });
    },
    [disablePluginMutation, enablePluginMutation, refreshData, runAction, stopPluginMutation, t],
  );

  const handleInstall = useCallback(
    async (marketPlugin: PluginMarketRecord) => {
      await runAction(
        marketPlugin.id,
        t('pages.plugins.toast.actionFailed', { name: marketPlugin.name }),
        async () => {
          await installPluginMutation.mutateAsync({
            body: {
              pluginId: marketPlugin.id,
              sourceId: marketPlugin.sourceId,
              version: marketPlugin.version,
            },
          });

          toast.success(
            marketPlugin.installedVersion && marketPlugin.updateAvailable
              ? t('pages.plugins.toast.updated', { name: marketPlugin.name })
              : t('pages.plugins.toast.installed', { name: marketPlugin.name }),
          );

          await refreshData();
        },
      );
    },
    [installPluginMutation, refreshData, runAction, t],
  );

  return {
    busyPluginId,
    dataErrorMessage: dataError ? getErrorMessage(dataError) : null,
    filteredMarketPlugins,
    filteredPlugins,
    handleInstall,
    handlePrimaryAction,
    handleToggleEnabled,
    hasSearchQuery,
    isDesktopTauri,
    loading,
    marketPlugins,
    plugins,
    refreshData,
    refreshing,
  };
}

export type PluginsPageState = ReturnType<typeof usePluginsPage>;
