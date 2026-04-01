import { useEffect, useRef } from 'react';
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
import {
  CATEGORY_OPTIONS,
  STATUS_OPTIONS,
  useHubModelCatalog,
} from './hooks/use-hub-model-catalog';

export default function Hub() {
  const hub = useHubModelCatalog();
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const loadMoreRef = useRef<HTMLDivElement | null>(null);
  usePageHeader(PAGE_HEADER_META.hub);

  const backendCount = new Set(hub.models.flatMap((model) => model.backend_ids)).size;

  useEffect(() => {
    if (!hub.hasMore || hub.isLoading) {
      return;
    }

    const root = scrollRef.current;
    const target = loadMoreRef.current;
    if (!root || !target) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          hub.loadMore();
        }
      },
      {
        root,
        rootMargin: '0px 0px 240px 0px',
      },
    );

    observer.observe(target);
    return () => observer.disconnect();
  }, [hub.hasMore, hub.isLoading, hub.loadMore]);

  return (
    <div ref={scrollRef} className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 px-1 pb-10">
        <section className="grid gap-6 xl:grid-cols-[minmax(0,1.9fr)_minmax(280px,0.92fr)]">
          <Card
            variant="hero"
            className="workspace-halo relative overflow-hidden rounded-[34px] border-none px-7 py-8 md:px-10 md:py-10"
          >
            <div className="absolute top-10 right-14 size-28 rounded-full bg-[color:color-mix(in_oklab,var(--brand-gold)_18%,var(--surface-1))] blur-3xl" />
            <div className="absolute right-[-5%] bottom-[-12%] size-56 rounded-full bg-[color:color-mix(in_oklab,var(--brand-teal)_16%,var(--surface-1))] blur-3xl" />

            <div className="relative flex h-full flex-col gap-8">
              <div className="space-y-4">
                <Badge
                  variant="chip"
                  className="border-transparent bg-[var(--shell-card)]/75 px-3 py-1 text-[10px] font-bold uppercase tracking-[0.22em] text-[var(--brand-gold)]"
                >
                  New release
                </Badge>
                <div className="space-y-4">
                  <h1 className="max-w-3xl text-4xl font-semibold tracking-[-0.05em] text-foreground md:text-6xl">
                    Shape your local <span className="text-[var(--brand-teal)]">model catalog.</span>
                  </h1>
                  <p className="max-w-2xl text-sm leading-7 text-muted-foreground md:text-lg">
                    Import JSON manifests, monitor runtime readiness, and keep every local inference
                    asset organized without leaving the workspace.
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
                  Import model
                </Button>
                <Button
                  variant="pill"
                  size="pill"
                  className="bg-[var(--shell-card)]/85 px-5"
                  onClick={() => void hub.refetch()}
                  disabled={hub.isRefetching}
                >
                  {hub.isRefetching ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <RefreshCw className="size-4" />
                  )}
                  Refresh catalog
                </Button>
              </div>
            </div>
          </Card>

          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-1">
            <HubSummaryCard
              icon={Boxes}
              value={hub.models.length}
              label="Catalog entries"
              description={`${backendCount || 0} backend${backendCount === 1 ? '' : 's'} currently mapped`}
              tone="gold"
            />
            <HubSummaryCard
              icon={HardDriveDownload}
              value={hub.downloadedCount}
              label="Ready locally"
              description={
                hub.pendingCount > 0
                  ? `${hub.pendingCount} download${hub.pendingCount === 1 ? '' : 's'} currently syncing`
                  : 'Catalog state is persisted locally for runtime pickup'
              }
              tone="blue"
            />
          </div>
        </section>

        <section className="space-y-4 rounded-[32px] border border-[var(--shell-card)]/70 bg-[var(--shell-card)]/45 px-4 py-4 shadow-[0_20px_48px_-42px_color-mix(in_oklab,var(--foreground)_30%,transparent)] backdrop-blur">
          <div className="flex flex-wrap items-center gap-2">
            {CATEGORY_OPTIONS.map((option) => {
              const isActive = hub.category === option.value;

              return (
                <Button
                  key={option.value}
                  variant={isActive ? 'cta' : 'pill'}
                  size="pill"
                  className="h-9 px-4 text-sm"
                  onClick={() => hub.setCategory(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}

            <Select
              value={hub.status}
              onValueChange={(value) => hub.setStatus(value as typeof hub.status)}
            >
              <SelectTrigger variant="pill" size="pill" className="h-9 min-w-[190px] bg-[var(--shell-card)]/85">
                <SelectValue placeholder="Status" />
              </SelectTrigger>
              <SelectContent variant="pill">
                {STATUS_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </section>

        {hub.error ? (
          <Alert variant="destructive">
            <TriangleAlert className="h-4 w-4" />
            <AlertTitle>Model catalog failed to load</AlertTitle>
            <AlertDescription>
              {String((hub.error as Error)?.message ?? hub.error)}
            </AlertDescription>
          </Alert>
        ) : null}

        {hub.isLoading ? (
          <StageEmptyState
            icon={Loader2}
            title="Loading model catalog"
            description="Fetching model entries and runtime status."
            className="[&_svg]:animate-spin"
          />
        ) : hub.filteredModels.length === 0 ? (
          <StageEmptyState
            icon={Boxes}
            title="No model entries match the current filters"
            description="Try another category, adjust status, or import a new model config."
            action={
              <Button variant="cta" size="pill" onClick={() => hub.setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                Import model
              </Button>
            }
          />
        ) : (
          <div className="space-y-4">
            <HubCatalogTable
              models={hub.visibleModels}
              deletePending={hub.deleteModelPending}
              onDeleteClick={hub.setModelToDelete}
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
  const iconClassName = tone === 'gold' ? 'text-[var(--brand-gold)]' : 'text-primary';

  return (
    <div
      className={`relative overflow-hidden rounded-[30px] border border-border/40 ${backgroundClassName} p-6 shadow-[0_24px_56px_-42px_color-mix(in_oklab,var(--foreground)_28%,transparent)]`}
    >
      <div className="absolute -top-5 -right-6 size-24 rounded-full bg-[var(--shell-card)]/45 blur-2xl" />
      <div className="relative flex h-full flex-col gap-6">
        <div
          className={`flex size-12 items-center justify-center rounded-[18px] bg-[var(--shell-card)]/75 ${iconClassName}`}
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
