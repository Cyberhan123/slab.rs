import { useEffect, useState } from 'react';
import { useIntersection } from '@mantine/hooks';
import { uniq } from 'lodash-es';
import { useTranslation } from '@slab/i18n';
import { useNavigate } from 'react-router-dom';
import {
  Boxes,
  HardDriveDownload,
  Loader2,
  Plus,
  RefreshCw,
  TriangleAlert,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { Card } from '@slab/components/card';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select';
import { StageEmptyState } from '@slab/components/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

import { HubCatalogTable } from './components/hub-catalog-table';
import { HubCreateModelDialog } from './components/hub-create-model-dialog';
import { HubDeleteModelDialog } from './components/hub-delete-model-dialog';
import { HubModelEnhancementSheet } from './components/hub-model-enhancement-sheet';
import {
  CATEGORY_OPTIONS,
  STATUS_OPTIONS,
  useHubModelCatalog,
} from './hooks/use-hub-model-catalog';

export default function Hub() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const hub = useHubModelCatalog();
  const { hasMore, isLoading, loadMore } = hub;
  const [scrollRoot, setScrollRoot] = useState<HTMLDivElement | null>(null);
  const { ref: loadMoreRef, entry: loadMoreEntry } = useIntersection<HTMLDivElement>({
    root: scrollRoot,
    rootMargin: '0px 0px 240px 0px',
  });
  usePageHeader({
    icon: PAGE_HEADER_META.hub.icon,
    title: t('pages.hub.header.title'),
    subtitle: t('pages.hub.header.subtitle'),
  });

  const backendCount = uniq(hub.models.flatMap((model) => model.backend_ids)).length;

  useEffect(() => {
    if (hasMore && !isLoading && loadMoreEntry?.isIntersecting) {
      loadMore();
    }
  }, [hasMore, isLoading, loadMore, loadMoreEntry?.isIntersecting]);

  return (
    <div ref={setScrollRoot} className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-1 pb-10">
        <section className="grid gap-6 xl:grid-cols-[minmax(0,1.9fr)_minmax(280px,0.92fr)]">
          <Card
            variant="hero"
            className="workspace-halo relative overflow-hidden rounded-3xl border-none px-7 py-8 md:px-10 md:py-10"
          >
            <div className="absolute top-10 right-14 size-28 rounded-full bg-[color:color-mix(in_oklab,var(--brand-gold)_18%,var(--surface-1))] blur-3xl" />
            <div className="absolute right-[-5%] bottom-[-12%] size-56 rounded-full bg-[color:color-mix(in_oklab,var(--brand-teal)_16%,var(--surface-1))] blur-3xl" />

            <div className="relative flex h-full flex-col gap-8">
              <div className="space-y-4">
                <Badge
                  variant="chip"
                  className="border-transparent bg-glass-bg-strong px-3 py-1 text-micro font-bold uppercase tracking-eyebrow text-[color:var(--brand-gold)]"
                >
                  {t('pages.hub.hero.badge')}
                </Badge>
                <div className="space-y-4">
                  <h1 className="max-w-3xl text-4xl font-semibold tracking-display text-foreground md:text-6xl">
                    {t('pages.hub.hero.titleLead')}{' '}
                    <span className="text-[color:var(--brand-teal)]">{t('pages.hub.hero.titleAccent')}</span>
                  </h1>
                  <p className="max-w-2xl text-sm leading-7 text-muted-foreground md:text-lg">
                    {t('pages.hub.hero.description')}
                  </p>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-3">
                <Button
                  variant="cta"
                  size="pill"
                  className="px-5"
                  onClick={() => hub.setCreateOpen(true)}
                >
                  <Plus className="size-4" />
                  {t('pages.hub.hero.importModel')}
                </Button>
                <Button
                  variant="pill"
                  size="pill"
                  className="bg-glass-bg-strong px-5"
                  onClick={() => void hub.refetch()}
                  disabled={hub.isRefetching}
                >
                  {hub.isRefetching ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw className="size-4" />
                  )}
                  {t('pages.hub.hero.refreshCatalog')}
                </Button>
              </div>
            </div>
          </Card>

          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-1">
            <HubSummaryCard
              icon={Boxes}
              value={hub.models.length}
              label={t('pages.hub.summary.catalogEntries')}
              description={t('pages.hub.summary.catalogDescription', { count: backendCount || 0 })}
              tone="gold"
            />
            <HubSummaryCard
              icon={HardDriveDownload}
              value={hub.downloadedCount}
              label={t('pages.hub.summary.readyLocally')}
              description={
                hub.pendingCount > 0
                  ? t('pages.hub.summary.pendingDescription', { count: hub.pendingCount })
                  : t('pages.hub.summary.readyDescription')
              }
              tone="blue"
            />
          </div>
        </section>

        <section className="space-y-4 rounded-3xl border border-[var(--shell-card)]/70 bg-glass-bg px-4 py-4 backdrop-blur">
          <div className="flex flex-wrap items-center gap-2">
            {CATEGORY_OPTIONS.map((option) => {
              const isActive = hub.category === option;

              return (
                <Button
                  key={option}
                  variant={isActive ? 'cta' : 'pill'}
                  size="pill"
                  className="h-9 px-4 text-sm"
                  onClick={() => hub.setCategory(option)}
                >
                  {t(`pages.hub.filters.categories.${option}`)}
                </Button>
              );
            })}

            <Select
              value={hub.status}
              onValueChange={(value) => hub.setStatus(value as typeof hub.status)}
            >
              <SelectTrigger variant="pill" size="pill" className="h-9 min-w-[190px] bg-glass-bg-strong">
                <SelectValue placeholder={t('pages.hub.filters.statusPlaceholder')} />
              </SelectTrigger>
              <SelectContent variant="pill">
                {STATUS_OPTIONS.map((option) => (
                  <SelectItem key={option} value={option}>
                    {t(`pages.hub.filters.statuses.${option}`)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </section>

        {hub.dataErrorMessage ? (
          <Alert variant="destructive">
            <TriangleAlert className="h-4 w-4" />
            <AlertTitle>{t('pages.hub.alerts.loadFailedTitle')}</AlertTitle>
            <AlertDescription>{hub.dataErrorMessage}</AlertDescription>
          </Alert>
        ) : null}

        {hub.isLoading ? (
          <StageEmptyState
            icon={Loader2}
            title={t('pages.hub.states.loadingTitle')}
            description={t('pages.hub.states.loadingDescription')}
            className="[&_svg]:animate-spin"
          />
        ) : hub.filteredModels.length === 0 ? (
          <StageEmptyState
            icon={Boxes}
            title={t('pages.hub.states.emptyFilteredTitle')}
            description={t('pages.hub.states.emptyFilteredDescription')}
            action={
              <Button variant="cta" size="pill" onClick={() => hub.setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                {t('pages.hub.hero.importModel')}
              </Button>
            }
          />
        ) : (
          <div className="space-y-4">
            <HubCatalogTable
              models={hub.visibleModels}
              deletePending={hub.deleteModelPending}
              modelActionPending={hub.modelActionPending}
              modelActionPendingId={hub.modelActionPendingId}
              modelActionErrors={hub.modelActionErrors}
              onDownloadClick={(model) => void hub.downloadModel(model)}
              onEnhanceClick={hub.setModelToEnhance}
              onDeleteClick={hub.setModelToDelete}
              onLoadClick={(model) => void hub.loadModel(model)}
              onSwitchClick={(model) => void hub.switchModel(model)}
              onUnloadClick={(model) => void hub.unloadModel(model)}
              onUseClick={(_model, route) => navigate(route)}
            />
            {hub.hasMore ? <div ref={loadMoreRef} className="h-8 w-full" aria-hidden="true" /> : null}
          </div>
        )}
      </div>

      <HubCreateModelDialog
        open={hub.isCreateOpen}
        onOpenChange={hub.setCreateOpen}
        selectedFileName={hub.createFileName}
        setCreateFile={hub.setCreateFile}
        canCreate={hub.canCreate}
        createPending={hub.createModelPending}
        onCreate={() => void hub.createModel()}
      />

      <HubDeleteModelDialog
        model={hub.modelToDelete}
        open={Boolean(hub.modelToDelete)}
        pending={hub.deleteModelPending}
        onOpenChange={(open) => {
          if (!open && !hub.deleteModelPending) {
            hub.setModelToDelete(null);
          }
        }}
        onConfirm={() => void hub.deleteModel()}
      />

      <HubModelEnhancementSheet
        model={hub.modelToEnhance}
        open={Boolean(hub.modelToEnhance)}
        onOpenChange={(open) => {
          if (!open) {
            hub.setModelToEnhance(null);
          }
        }}
        onSaved={() => void hub.refetch()}
      />
    </div>
  );
}

function HubSummaryCard({
  icon: Icon,
  value,
  label,
  description,
  tone,
}: {
  icon: typeof Boxes;
  value: number;
  label: string;
  description: string;
  tone: 'gold' | 'blue';
}) {
  const backgroundClassName =
    tone === 'gold'
      ? 'bg-[linear-gradient(180deg,color-mix(in_oklab,var(--brand-gold)_12%,var(--surface-1))_0%,var(--surface-1)_100%)]'
      : 'bg-[linear-gradient(180deg,color-mix(in_oklab,var(--primary)_12%,var(--surface-1))_0%,var(--surface-1)_100%)]';
  const iconClassName = tone === 'gold' ? 'text-[color:var(--brand-gold)]' : 'text-primary';

  return (
    <div
      className={`relative overflow-hidden rounded-3xl border border-border/40 ${backgroundClassName} p-6`}
    >
      <div className="absolute -top-5 -right-6 size-24 rounded-full bg-glass-bg blur-2xl" />
      <div className="relative flex h-full flex-col gap-6">
        <div
          className={`flex size-12 items-center justify-center rounded-[18px] bg-glass-bg-strong ${iconClassName}`}
        >
          <Icon className="size-5" />
        </div>
        <div className="space-y-1">
          <p className="text-4xl font-semibold tracking-tight text-foreground">{value}</p>
          <p className="text-[1.65rem] font-semibold tracking-tight text-foreground">{label}</p>
          <p className="text-sm leading-6 text-muted-foreground">{description}</p>
        </div>
      </div>
    </div>
  );
}
