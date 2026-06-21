import {
  Loader2,
  PackagePlus,
  Link,
  PlugZap,
  RefreshCw,
  Search,
  TriangleAlert,
} from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Button } from '@slab/components/button';
import type { PluginsPageState } from '../hooks/use-plugins-page';
import { PLUGIN_ICONS, PLUGIN_TONES } from '../utils';
import { EmptyPanel } from './empty-panel';
import { ImportPluginPackDialog } from './import-plugin-pack-dialog';
import { InstallPluginUrlDialog } from './install-plugin-url-dialog';
import { InstalledPluginCard } from './installed-plugin-card';
import { InstalledSkeletonGrid } from './plugin-skeletons';
import { SectionHeading } from './section-heading';

export function PluginsWorkbench({
  busyPluginId,
  canImport,
  canInstallFromUrl,
  dataErrorMessage,
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
  importFileName,
  importPluginPending,
  importUploadProgress,
  importPreview,
  importPreviewFailed,
  installPluginPending,
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
}: PluginsPageState) {
  const { t } = useTranslation();

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-8 px-1 pb-10">
        {dataErrorMessage ? (
          <Alert variant="destructive">
            <TriangleAlert className="h-4 w-4" />
            <AlertTitle>{t('pages.plugins.alerts.loadFailedTitle')}</AlertTitle>
            <AlertDescription>{dataErrorMessage}</AlertDescription>
          </Alert>
        ) : null}

        <section className="space-y-4">
          <SectionHeading
            icon={PlugZap}
            title={t('pages.plugins.sections.installed')}
            action={
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleImportOpenChange(true)}
                  disabled={importPluginPending}
                  className="rounded-[12px] bg-[var(--shell-card)]/80"
                  data-testid="plugin-import-open-button"
                >
                  {importPluginPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <PackagePlus className="size-4" />
                  )}
                  {t('pages.plugins.actions.import')}
                </Button>
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => handleUrlInstallOpenChange(true)}
                  disabled={installPluginPending}
                  className="rounded-[12px] bg-[var(--shell-card)]/80"
                  data-testid="plugin-url-install-open-button"
                >
                  {installPluginPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Link className="size-4" />
                  )}
                  {t('pages.plugins.actions.installFromUrl')}
                </Button>
                <Button
                  variant="pill"
                  size="sm"
                  onClick={() => void refreshData()}
                  disabled={refreshing}
                  className="rounded-[12px] bg-[var(--shell-card)]/80"
                  data-testid="plugin-refresh-button"
                >
                  {refreshing ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw className="size-4" />
                  )}
                  {t('pages.plugins.actions.refresh')}
                </Button>
              </div>
            }
          />

          {loading && plugins.length === 0 ? (
            <InstalledSkeletonGrid />
          ) : plugins.length === 0 ? (
            <EmptyPanel
              icon={PlugZap}
              title={t('pages.plugins.empty.noInstalled.title')}
              description={t('pages.plugins.empty.noInstalled.description')}
            />
          ) : hasSearchQuery && filteredPlugins.length === 0 ? (
            <EmptyPanel
              icon={Search}
              title={t('pages.plugins.empty.noInstalledMatches.title')}
              description={t('pages.plugins.empty.noInstalledMatches.description')}
            />
          ) : (
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
              {filteredPlugins.map((plugin, index) => (
                <InstalledPluginCard
                  key={plugin.id}
                  plugin={plugin}
                  icon={PLUGIN_ICONS[index % PLUGIN_ICONS.length]}
                  tone={PLUGIN_TONES[index % PLUGIN_TONES.length]}
                  busy={busyPluginId === plugin.id}
                  actionError={pluginActionErrors[plugin.id] ?? null}
                  onPrimaryAction={() => void handlePrimaryAction(plugin)}
                  onToggleEnabled={() => void handleToggleEnabled(plugin)}
                  onUpdate={() => void handleUpdatePlugin(plugin)}
                  onDelete={() => void handleDeletePlugin(plugin)}
                />
              ))}
            </div>
          )}
        </section>
      </div>

      <ImportPluginPackDialog
        open={isImportOpen}
        onOpenChange={handleImportOpenChange}
        selectedFileName={importFileName}
        setImportFile={handleImportFileChange}
        canImport={canImport}
        importPending={importPluginPending}
        importUploadProgress={importUploadProgress}
        onCancelImport={handleCancelImport}
        onImport={() => void handleImportPlugin()}
        importPreview={importPreview}
        importPreviewFailed={importPreviewFailed}
        hasReviewedPermissions={hasReviewedPermissions}
        onReviewedPermissionsChange={setHasReviewedPermissions}
      />

      <InstallPluginUrlDialog
        open={isUrlInstallOpen}
        onOpenChange={handleUrlInstallOpenChange}
        pluginId={urlInstallPluginId}
        packageUrl={urlInstallPackageUrl}
        packageSha256={urlInstallPackageSha256}
        version={urlInstallVersion}
        pending={installPluginPending}
        canInstall={canInstallFromUrl}
        onPluginIdChange={setUrlInstallPluginId}
        onPackageUrlChange={setUrlInstallPackageUrl}
        onPackageSha256Change={setUrlInstallPackageSha256}
        onVersionChange={setUrlInstallVersion}
        onInstall={() => void handleInstallFromUrl()}
      />
    </div>
  );
}
