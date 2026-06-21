import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { usePageHeader, usePageHeaderSearch } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';
import api, { getLocalizedErrorMessage, postFormData } from '@slab/api';
import {
  isPluginRunning,
  pluginSearchText,
  type PluginRecord,
} from '../utils';
import { parsePluginPackManifest, type PluginManifestPreview } from '../lib/plugin-manifest-preview';
import { connectPluginEvents } from '../lib/plugin-runtime-client';
import { RUNTIME_PLUGINS_QUERY_KEY } from './use-runtime-plugins';

type ImportedPluginResponse = PluginRecord;
type PluginActionError = {
  message: string;
  error: unknown;
};

async function importPluginPack(
  file: File,
  invalidFileMessage: string,
  options: Parameters<typeof postFormData>[2] = {},
): Promise<ImportedPluginResponse> {
  if (!isPluginPackFile(file)) {
    throw new Error(invalidFileMessage);
  }

  return postFormData('/v1/plugins/import-pack', file, options);
}

export function usePluginsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  usePageHeader({
    icon: PAGE_HEADER_META.plugins.icon,
    title: t('pages.plugins.header.title'),
    subtitle: t('pages.plugins.header.subtitle'),
  });

  const [busyPluginId, setBusyPluginId] = useState<string | null>(null);
  const [pluginActionErrors, setPluginActionErrors] = useState<Record<string, PluginActionError>>({});
  const [searchQuery, setSearchQuery] = useState('');
  const [isImportOpen, setIsImportOpen] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);
  const [importPluginPending, setImportPluginPending] = useState(false);
  const [importUploadProgress, setImportUploadProgress] = useState<number | null>(null);
  const [importPreview, setImportPreview] = useState<PluginManifestPreview | null>(null);
  const [importPreviewFailed, setImportPreviewFailed] = useState(false);
  const [hasReviewedPermissions, setHasReviewedPermissions] = useState(false);
  const [isUrlInstallOpen, setIsUrlInstallOpen] = useState(false);
  const [urlInstallPluginId, setUrlInstallPluginId] = useState('');
  const [urlInstallPackageUrl, setUrlInstallPackageUrl] = useState('');
  const [urlInstallPackageSha256, setUrlInstallPackageSha256] = useState('');
  const [urlInstallVersion, setUrlInstallVersion] = useState('');
  const importAbortRef = useRef<AbortController | null>(null);
  const seenPluginEventsRef = useRef<Set<string>>(new Set());

  const headerSearch = useMemo(
    () => ({
      type: 'search' as const,
      value: searchQuery,
      onValueChange: setSearchQuery,
      placeholder: t('pages.plugins.search.placeholder'),
      ariaLabel: t('pages.plugins.search.ariaLabel'),
    }),
    [searchQuery, t],
  );

  usePageHeaderSearch(headerSearch);

  const {
    data: pluginRows,
    error: pluginsError,
    isLoading: pluginsLoading,
    isFetching: pluginsFetching,
    refetch: refetchPlugins,
  } = api.useQuery('get', '/v1/plugins');
  const enablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/enable', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const disablePluginMutation = api.useMutation('post', '/v1/plugins/{id}/disable', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const startPluginMutation = api.useMutation('post', '/v1/plugins/{id}/start', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const stopPluginMutation = api.useMutation('post', '/v1/plugins/{id}/stop', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const installPluginMutation = api.useMutation('post', '/v1/plugins/install', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });
  const deletePluginMutation = api.useMutation('delete', '/v1/plugins/{id}', {
    meta: {
      skipGlobalErrorToast: true,
    },
  });

  const plugins = useMemo(() => pluginRows ?? [], [pluginRows]);
  const loading = pluginsLoading;
  const refreshing = pluginsFetching;
  const dataError = pluginsError;
  // Require an explicit "I've reviewed the permissions" acknowledgement before
  // installing, so users never import a pack without at least seeing its asks.
  const canImport = Boolean(importFile && !importPluginPending && hasReviewedPermissions);
  const canInstallFromUrl =
    urlInstallPluginId.trim().length > 0 &&
    urlInstallPackageUrl.trim().length > 0 &&
    !installPluginMutation.isPending;

  const deferredSearchQuery = useDeferredValue(searchQuery);
  const normalizedSearchQuery = deferredSearchQuery.trim().toLowerCase();
  const hasSearchQuery = normalizedSearchQuery.length > 0;

  const filteredPlugins = useMemo(() => {
    if (!normalizedSearchQuery) return plugins;
    return plugins.filter((plugin) => pluginSearchText(plugin).includes(normalizedSearchQuery));
  }, [normalizedSearchQuery, plugins]);

  const refreshData = useCallback(async () => {
    try {
      await Promise.all([
        refetchPlugins(),
        queryClient.invalidateQueries({ queryKey: RUNTIME_PLUGINS_QUERY_KEY }),
        queryClient.invalidateQueries({
          predicate: (query) => JSON.stringify(query.queryKey).includes('/v1/plugins'),
        }),
      ]);
    } catch (error) {
      toast.error(t('pages.plugins.toast.loadFailed'), {
        description: getLocalizedErrorMessage(error, t),
      });
    }
  }, [queryClient, refetchPlugins, t]);

  useEffect(() => {
    seenPluginEventsRef.current.clear();
    return connectPluginEvents({
      onEvent: (event) => {
        const key = `${event.plugin_id}:${event.topic}:${event.ts}`;
        if (seenPluginEventsRef.current.has(key)) {
          return;
        }
        seenPluginEventsRef.current.add(key);
        void refreshData();
      },
    });
  }, [refreshData]);

  const runAction = useCallback(
    async (pluginId: string, errorTitle: string, action: () => Promise<void>) => {
      setBusyPluginId(pluginId);
      setPluginActionErrors((current) => {
        const next = { ...current };
        delete next[pluginId];
        return next;
      });
      try {
        await action();
      } catch (error) {
        const message = getLocalizedErrorMessage(error, t);
        setPluginActionErrors((current) => ({
          ...current,
          [pluginId]: {
            error,
            message,
          },
        }));
        toast.error(errorTitle, {
          description: message,
        });
      } finally {
        setBusyPluginId(null);
      }
    },
    [t],
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
            // Intentionally omit `lastError`. The backend preserves the existing
            // diagnostic; sending `null` here used to clear a prior start/runtime
            // failure so the reason vanished after a manual stop.
            body: {},
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
              // Omit `lastError` so the backend keeps the prior failure diagnostic.
              body: {},
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
    setImportPreview(null);
    setImportPreviewFailed(false);
    setHasReviewedPermissions(false);
    setImportUploadProgress(null);
    importAbortRef.current?.abort();
    importAbortRef.current = null;
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
    setHasReviewedPermissions(false);
    setImportPreview(null);
    setImportPreviewFailed(false);

    if (!file) {
      return;
    }

    // Parse plugin.json out of the pack client-side so the user can review the
    // requested permissions before it is uploaded. Failure is non-fatal: we surface
    // a notice and still allow the import once the user acknowledges.
    void parsePluginPackManifest(file)
      .then((preview) => {
        if (!preview || preview.parseError) {
          setImportPreviewFailed(true);
          setImportPreview(null);
        } else {
          setImportPreview(preview);
        }
      })
      .catch(() => {
        setImportPreviewFailed(true);
        setImportPreview(null);
      });
  }, []);

  const handleImportPlugin = useCallback(async () => {
    if (!importFile || importPluginPending) {
      return;
    }

    const abortController = new AbortController();
    importAbortRef.current = abortController;
    setImportPluginPending(true);
    setImportUploadProgress(0);
    try {
      const imported = await importPluginPack(importFile, t('pages.plugins.error.onlyPluginPacks'), {
        signal: abortController.signal,
        onUploadProgress: ({ loaded, total }) => {
          if (!total || total <= 0) {
            setImportUploadProgress(null);
            return;
          }
          setImportUploadProgress(Math.min(100, Math.max(0, (loaded / total) * 100)));
        },
      });

      toast.success(t('pages.plugins.toast.imported', { name: imported.name }), {
        description: imported.name,
      });

      handleImportOpenChange(false);
      await refreshData();
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        toast.message(t('pages.plugins.toast.importCancelled'));
        return;
      }

      toast.error(t('pages.plugins.toast.importFailed'), {
        description: getLocalizedErrorMessage(error, t),
      });
    } finally {
      importAbortRef.current = null;
      setImportPluginPending(false);
      setImportUploadProgress(null);
    }
  }, [handleImportOpenChange, importFile, importPluginPending, refreshData, t]);

  const handleCancelImport = useCallback(() => {
    importAbortRef.current?.abort();
  }, []);

  const resetUrlInstallState = useCallback(() => {
    setUrlInstallPluginId('');
    setUrlInstallPackageUrl('');
    setUrlInstallPackageSha256('');
    setUrlInstallVersion('');
  }, []);

  const handleUrlInstallOpenChange = useCallback(
    (open: boolean) => {
      setIsUrlInstallOpen(open);
      if (!open && !installPluginMutation.isPending) {
        resetUrlInstallState();
      }
    },
    [installPluginMutation.isPending, resetUrlInstallState],
  );

  const handleInstallFromUrl = useCallback(async () => {
    if (!canInstallFromUrl) {
      return;
    }

    try {
      const installed = await installPluginMutation.mutateAsync({
        body: {
          pluginId: urlInstallPluginId.trim(),
          packageUrl: urlInstallPackageUrl.trim(),
          packageSha256: urlInstallPackageSha256.trim() || undefined,
          version: urlInstallVersion.trim() || undefined,
        },
      });
      toast.success(t('pages.plugins.toast.installed', { name: installed.name }), {
        description: installed.name,
      });
      handleUrlInstallOpenChange(false);
      await refreshData();
    } catch (error) {
      toast.error(t('pages.plugins.toast.installFailed'), {
        description: getLocalizedErrorMessage(error, t),
      });
    }
  }, [
    canInstallFromUrl,
    handleUrlInstallOpenChange,
    installPluginMutation,
    refreshData,
    t,
    urlInstallPackageSha256,
    urlInstallPackageUrl,
    urlInstallPluginId,
    urlInstallVersion,
  ]);

  const handleUpdatePlugin = useCallback(
    async (plugin: PluginRecord) => {
      if (!plugin.updateAvailable || !plugin.sourceRef?.trim()) {
        return;
      }

      await runAction(plugin.id, t('pages.plugins.toast.actionFailed', { name: plugin.name }), async () => {
        const updated = await installPluginMutation.mutateAsync({
          body: {
            pluginId: plugin.id,
            packageUrl: plugin.sourceRef?.trim(),
            version: plugin.availableVersion ?? undefined,
          },
        });
        toast.success(t('pages.plugins.toast.updated', { name: updated.name }));
        await refreshData();
      });
    },
    [installPluginMutation, refreshData, runAction, t],
  );

  const handleDeletePlugin = useCallback(
    async (plugin: PluginRecord) => {
      if (!plugin.removable) {
        return;
      }

      await runAction(plugin.id, t('pages.plugins.toast.actionFailed', { name: plugin.name }), async () => {
        await deletePluginMutation.mutateAsync({
          params: {
            path: { id: plugin.id },
          },
        });
        toast.success(t('pages.plugins.toast.uninstalled', { name: plugin.name }));
        await refreshData();
      });
    },
    [deletePluginMutation, refreshData, runAction, t],
  );

  return {
    busyPluginId,
    canImport,
    canInstallFromUrl,
    dataErrorMessage: dataError ? getLocalizedErrorMessage(dataError, t) : null,
    filteredPlugins,
    handleCancelImport,
    handleDeletePlugin,
    handleImportFileChange,
    handleImportOpenChange,
    handleImportPlugin,
    handleInstallFromUrl,
    handlePrimaryAction,
    handleToggleEnabled,
    handleUpdatePlugin,
    handleUrlInstallOpenChange,
    hasReviewedPermissions,
    hasSearchQuery,
    importFileName: importFile?.name ?? null,
    importPluginPending,
    importUploadProgress,
    importPreview,
    importPreviewFailed,
    installPluginPending: installPluginMutation.isPending,
    isUrlInstallOpen,
    isImportOpen,
    loading,
    pluginActionErrors,
    plugins,
    refreshData,
    refreshing,
    setHasReviewedPermissions,
    setUrlInstallPackageSha256,
    setUrlInstallPackageUrl,
    setUrlInstallPluginId,
    setUrlInstallVersion,
    urlInstallPackageSha256,
    urlInstallPackageUrl,
    urlInstallPluginId,
    urlInstallVersion,
  };
}

export type PluginsPageState = ReturnType<typeof usePluginsPage>;

function isPluginPackFile(file: File) {
  return file.name.trim().toLowerCase().endsWith('.plugin.slab');
}
