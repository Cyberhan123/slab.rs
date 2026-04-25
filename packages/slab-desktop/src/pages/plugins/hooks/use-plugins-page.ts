import { useCallback, useDeferredValue, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePageHeader, usePageHeaderSearch } from '@/hooks/use-global-header-meta';
import { isTauri } from '@/hooks/use-tauri';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api, { getErrorMessage } from '@slab/api';
import { SERVER_BASE_URL } from '@slab/api/config';
import {
  isPluginRunning,
  pluginSearchText,
  type PluginRecord,
} from '../utils';

type ImportedPluginResponse = PluginRecord;

async function parseErrorPayload(response: Response): Promise<unknown> {
  try {
    return await response.clone().json();
  } catch {
    try {
      return await response.clone().text();
    } catch {
      return undefined;
    }
  }
}

async function importPluginPack(body: FormData): Promise<ImportedPluginResponse> {
  const response = await fetch(`${SERVER_BASE_URL}/v1/plugins/import-pack`, {
    body,
    method: 'POST',
  });

  if (!response.ok) {
    throw createImportError(response, await parseErrorPayload(response));
  }

  return (await response.json()) as ImportedPluginResponse;
}

function createImportError(response: Response, payload: unknown) {
  if (payload instanceof Error) {
    return payload;
  }

  if (typeof payload === 'string' && payload.trim().length > 0) {
    return new Error(payload);
  }

  if (payload && typeof payload === 'object') {
    const candidate = payload as { error?: unknown; message?: unknown };
    if (typeof candidate.error === 'string' && candidate.error.trim().length > 0) {
      return new Error(candidate.error);
    }
    if (typeof candidate.message === 'string' && candidate.message.trim().length > 0) {
      return new Error(candidate.message);
    }
  }

  return new Error(`Request failed with ${response.status}`);
}

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
  const [isImportOpen, setIsImportOpen] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importPluginPending, setImportPluginPending] = useState(false);

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
  const enablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/enable');
  const disablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/disable');
  const startPluginMutation = api.useMutation('post', '/v1/plugins/{id}/start');
  const stopPluginMutation = api.useMutation('post', '/v1/plugins/{id}/stop');

  const plugins = useMemo(() => pluginRows ?? [], [pluginRows]);
  const loading = pluginsLoading;
  const refreshing = pluginsFetching;
  const dataError = pluginsError;
  const canImport = Boolean(importFile && !importPluginPending);

  const deferredSearchQuery = useDeferredValue(searchQuery);
  const normalizedSearchQuery = deferredSearchQuery.trim().toLowerCase();
  const hasSearchQuery = normalizedSearchQuery.length > 0;

  const filteredPlugins = useMemo(() => {
    if (!normalizedSearchQuery) return plugins;
    return plugins.filter((plugin) => pluginSearchText(plugin).includes(normalizedSearchQuery));
  }, [normalizedSearchQuery, plugins]);

  const refreshData = useCallback(async () => {
    if (!isDesktopTauri) return;

    try {
      await refetchPlugins();
    } catch (error) {
      toast.error(t('pages.plugins.toast.loadFailed'), {
        description: getErrorMessage(error),
      });
    }
  }, [isDesktopTauri, refetchPlugins, t]);

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

  const resetImportState = useCallback(() => {
    setImportFile(null);
  }, []);

  const handleImportOpenChange = useCallback(
    (open: boolean) => {
      setIsImportOpen(open);
      if (!open && !importPluginPending) {
        resetImportState();
      }
    },
    [importPluginPending, resetImportState],
  );

  const handleImportFileChange = useCallback((file: File | null) => {
    setImportFile(file);
  }, []);

  const handleImportPlugin = useCallback(async () => {
    if (!importFile || importPluginPending) {
      return;
    }

    setImportPluginPending(true);
    try {
      const imported = await importPluginPack(
        buildImportPluginPackBody(importFile, t('pages.plugins.error.onlyPluginPacks')),
      );

      toast.success(t('pages.plugins.toast.imported', { name: imported.name }), {
        description: imported.name,
      });

      handleImportOpenChange(false);
      await refreshData();
    } catch (error) {
      toast.error(t('pages.plugins.toast.importFailed'), {
        description: getErrorMessage(error),
      });
    } finally {
      setImportPluginPending(false);
    }
  }, [handleImportOpenChange, importFile, importPluginPending, refreshData, t]);

  return {
    busyPluginId,
    canImport,
    dataErrorMessage: dataError ? getErrorMessage(dataError) : null,
    filteredPlugins,
    handleImportFileChange,
    handleImportOpenChange,
    handleImportPlugin,
    handlePrimaryAction,
    handleToggleEnabled,
    hasSearchQuery,
    importFileName: importFile?.name ?? null,
    importPluginPending,
    isImportOpen,
    isDesktopTauri,
    loading,
    plugins,
    refreshData,
    refreshing,
  };
}

export type PluginsPageState = ReturnType<typeof usePluginsPage>;

function isPluginPackFile(file: File) {
  return file.name.trim().toLowerCase().endsWith('.plugin.slab');
}

function buildImportPluginPackBody(file: File, invalidFileMessage: string) {
  if (!isPluginPackFile(file)) {
    throw new Error(invalidFileMessage);
  }

  const body = new FormData();
  body.set('file', file, file.name);
  return body;
}
