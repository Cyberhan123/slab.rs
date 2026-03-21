import {
  Boxes,
  HardDriveDownload,
  Loader2,
  Plus,
  RefreshCw,
  Search,
  TriangleAlert,
} from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  MetricCard,
  PillFilterBar,
  SoftPanel,
  StageEmptyState,
} from '@/components/ui/workspace';
import { usePageHeader } from '@/hooks/use-global-header-meta';
import { PAGE_HEADER_META } from '@/layouts/header-meta';

import { HubCatalogTable } from './components/hub-catalog-table';
import { HubCreateModelDialog } from './components/hub-create-model-dialog';
import { HubDeleteModelDialog } from './components/hub-delete-model-dialog';
import {
  PAGE_SIZE_OPTIONS,
  STATUS_OPTIONS,
  useHubModelCatalog,
} from './hooks/use-hub-model-catalog';

export default function Hub() {
  const hub = useHubModelCatalog();
  usePageHeader(PAGE_HEADER_META.hub);

  return (
    <div className="h-full w-full overflow-y-auto">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-1 pb-8">
        <SoftPanel className="workspace-halo space-y-5 overflow-hidden rounded-[30px] border border-border/70">
          <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
            <div className="space-y-2">
              <Badge variant="chip">Model operations center</Badge>
              <h1 className="text-2xl font-semibold tracking-tight md:text-3xl">
                Model Hub
              </h1>
              <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                Import model config JSON files, monitor download readiness, and manage runtime catalog
                entries without leaving the workspace.
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="pill"
                size="pill"
                onClick={() => void hub.refetch()}
                disabled={hub.isRefetching}
              >
                {hub.isRefetching ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <RefreshCw className="mr-2 h-4 w-4" />
                )}
                Refresh
              </Button>
              <Button variant="cta" size="pill" onClick={() => hub.setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                Import model
              </Button>
            </div>
          </div>

          <div className="grid gap-3 md:grid-cols-3">
            <MetricCard
              label="Catalog Models"
              value={hub.models.length}
              hint="Total entries available for runtime use"
              icon={Boxes}
            />
            <MetricCard
              label="Downloaded"
              value={hub.downloadedCount}
              hint="Models with local paths ready for inference"
              icon={HardDriveDownload}
            />
            <MetricCard
              label="Downloading"
              value={hub.pendingCount}
              hint="Background download tasks currently in progress"
              icon={Loader2}
            />
          </div>
        </SoftPanel>

        <PillFilterBar>
          <div className="relative min-w-[260px] flex-1">
            <Search className="pointer-events-none absolute top-1/2 left-4 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              variant="shell"
              value={hub.search}
              onChange={(event) => hub.setSearch(event.target.value)}
              placeholder="Search by model, repo, filename, backend, or status"
              className="h-10 w-full pl-10"
            />
          </div>

          <Select
            value={hub.status}
            onValueChange={(value) => hub.setStatus(value as typeof hub.status)}
          >
            <SelectTrigger variant="pill" size="pill" className="min-w-[190px]">
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

          <Select
            value={String(hub.pageSize)}
            onValueChange={(value) =>
              hub.setPageSize(Number(value) as (typeof PAGE_SIZE_OPTIONS)[number])
            }
          >
            <SelectTrigger variant="pill" size="pill" className="min-w-[140px]">
              <SelectValue placeholder="Rows" />
            </SelectTrigger>
            <SelectContent variant="pill">
              {PAGE_SIZE_OPTIONS.map((size) => (
                <SelectItem key={size} value={String(size)}>
                  {size} rows
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </PillFilterBar>

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
            title="No model entries match the current filter"
            description="Try another search term, change status filter, or import a new model config."
            action={
              <Button variant="cta" size="pill" onClick={() => hub.setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" />
                Import model
              </Button>
            }
          />
        ) : (
          <SoftPanel className="space-y-4 p-3">
            <HubCatalogTable
              models={hub.pagedModels}
              deletePending={hub.deleteModelPending}
              onDeleteClick={hub.setModelToDelete}
            />

            <div className="flex flex-col gap-3 px-1 pb-1 sm:flex-row sm:items-center sm:justify-between">
              <p className="text-sm text-muted-foreground">
                Showing {hub.showingFrom}-{hub.showingTo} of {hub.filteredModels.length}
              </p>
              <div className="flex items-center gap-2">
                <Button
                  variant="pill"
                  size="sm"
                  onClick={() => hub.setPage((value) => value - 1)}
                  disabled={hub.page <= 1}
                >
                  Previous
                </Button>
                <Badge variant="counter">
                  Page {hub.page} / {hub.totalPages}
                </Badge>
                <Button
                  variant="pill"
                  size="sm"
                  onClick={() => hub.setPage((value) => value + 1)}
                  disabled={hub.page >= hub.totalPages}
                >
                  Next
                </Button>
              </div>
            </div>
          </SoftPanel>
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
