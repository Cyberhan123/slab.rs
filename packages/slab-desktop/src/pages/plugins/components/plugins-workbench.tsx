import {
  Loader2,
  PackageOpen,
  PlugZap,
  RefreshCw,
  Search,
  TriangleAlert,
} from 'lucide-react';
import { useTranslation } from '@slab/i18n';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Button } from '@slab/components/button';
import { StageEmptyState } from '@slab/components/workspace';
import type { PluginsPageState } from '../hooks/use-plugins-page';
import { MARKET_ICONS, PLUGIN_ICONS, PLUGIN_TONES } from '../utils';
import { EmptyPanel } from './empty-panel';
import { InstalledPluginCard } from './installed-plugin-card';
import { MarketPluginRow } from './market-plugin-row';
import { InstalledSkeletonGrid, MarketSkeletonRow } from './plugin-skeletons';
import { SectionHeading } from './section-heading';

export function PluginsWorkbench({
  busyPluginId,
  dataErrorMessage,
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
}: PluginsPageState) {
  const { t } = useTranslation();

  if (!isDesktopTauri) {
    return (
      <div className="h-full w-full overflow-y-auto px-1 pb-10">
        <StageEmptyState
          icon={PackageOpen}
          title={t('pages.plugins.desktopOnly.title')}
          description={t('pages.plugins.desktopOnly.description')}
          className="min-h-[520px]"
        />
      </div>
    );
  }

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
              <Button
                variant="pill"
                size="sm"
                onClick={() => void refreshData()}
                disabled={refreshing}
                className="rounded-[12px] bg-[var(--shell-card)]/80"
              >
                {refreshing ? <Loader2 className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
                {t('pages.plugins.actions.refresh')}
              </Button>
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
                  onPrimaryAction={() => void handlePrimaryAction(plugin)}
                  onToggleEnabled={() => void handleToggleEnabled(plugin)}
                />
              ))}
            </div>
          )}
        </section>

        <section className="space-y-4">
          <SectionHeading
            icon={PackageOpen}
            title={t('pages.plugins.sections.market')}
          />

          <div className="space-y-3">
            {loading && marketPlugins.length === 0 ? (
              <>
                <MarketSkeletonRow />
                <MarketSkeletonRow />
              </>
            ) : marketPlugins.length === 0 ? (
              <EmptyPanel
                icon={PackageOpen}
                title={t('pages.plugins.empty.noMarket.title')}
                description={t('pages.plugins.empty.noMarket.description')}
              />
            ) : filteredMarketPlugins.length === 0 ? (
              <EmptyPanel
                icon={Search}
                title={t('pages.plugins.empty.noMarketMatches.title')}
                description={t('pages.plugins.empty.noMarketMatches.description')}
              />
            ) : (
              filteredMarketPlugins.map((plugin, index) => (
                <MarketPluginRow
                  key={`${plugin.sourceId}:${plugin.id}`}
                  plugin={plugin}
                  icon={MARKET_ICONS[index % MARKET_ICONS.length]}
                  busy={busyPluginId === plugin.id}
                  onInstall={() => void handleInstall(plugin)}
                />
              ))
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
