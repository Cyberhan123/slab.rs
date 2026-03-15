import { Loader2, Search, TriangleAlert } from 'lucide-react';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';

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

  return (
    <div className="h-full overflow-y-auto px-4 py-4">
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-6 pb-10">
        <Card className="border-border/70 shadow-[0_20px_60px_-48px_color-mix(in_oklab,var(--foreground)_32%,transparent)]">
          <CardHeader className="gap-4 border-b border-border/60">
            <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_220px_140px]">
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={hub.search}
                  onChange={(event) => hub.setSearch(event.target.value)}
                  placeholder="Search by model name, repo, filename, backend, or task"
                  className="pl-9"
                />
              </div>

              <Select value={hub.status} onValueChange={(value) => hub.setStatus(value as typeof hub.status)}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Status" />
                </SelectTrigger>
                <SelectContent>
                  {STATUS_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>

              <Select
                value={String(hub.pageSize)}
                onValueChange={(value) => hub.setPageSize(Number(value) as (typeof PAGE_SIZE_OPTIONS)[number])}
              >
                <SelectTrigger className="w-full">
                  <SelectValue placeholder="Rows" />
                </SelectTrigger>
                <SelectContent>
                  {PAGE_SIZE_OPTIONS.map((size) => (
                    <SelectItem key={size} value={String(size)}>
                      {size} rows
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardHeader>

          <CardContent className="space-y-4 pt-6">
            {hub.error ? (
              <Alert variant="destructive">
                <TriangleAlert className="h-4 w-4" />
                <AlertTitle>Model catalog failed to load</AlertTitle>
                <AlertDescription>{String((hub.error as Error)?.message ?? hub.error)}</AlertDescription>
              </Alert>
            ) : null}

            {hub.isLoading ? (
              <div className="flex min-h-[240px] items-center justify-center">
                <div className="flex items-center gap-3 rounded-full border border-border/60 px-5 py-3 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Loading model catalog...
                </div>
              </div>
            ) : hub.pagedModels.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-border/70 px-6 py-12 text-center">
                <p className="font-medium">
                  {hub.filteredModels.length === 0
                    ? 'No model entries match the current filter.'
                    : 'No rows on this page.'}
                </p>
                <p className="mt-2 text-sm text-muted-foreground">
                  Try another search term, change the status filter, or add a new model entry.
                </p>
              </div>
            ) : (
              <>
                <HubCatalogTable
                  models={hub.pagedModels}
                  deletePending={hub.deleteModelPending}
                  onDeleteClick={hub.setModelToDelete}
                />

                <div className="flex flex-col gap-3 border-t border-border/60 pt-4 sm:flex-row sm:items-center sm:justify-between">
                  <p className="text-sm text-muted-foreground">
                    Showing {hub.showingFrom}-{hub.showingTo} of {hub.filteredModels.length}
                  </p>
                  <div className="flex items-center gap-2">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => hub.setPage((value) => value - 1)}
                      disabled={hub.page <= 1}
                    >
                      Previous
                    </Button>
                    <Badge variant="outline">
                      Page {hub.page} / {hub.totalPages}
                    </Badge>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => hub.setPage((value) => value + 1)}
                      disabled={hub.page >= hub.totalPages}
                    >
                      Next
                    </Button>
                  </div>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>

      <HubCreateModelDialog
        open={hub.isCreateOpen}
        onOpenChange={hub.setCreateOpen}
        form={hub.form}
        setField={hub.setField}
        toggleBackend={hub.toggleBackend}
        repoLookupRepoId={hub.repoLookup?.repo_id}
        repoLookupFilter={hub.repoLookupFilter}
        setRepoLookupFilter={hub.setRepoLookupFilter}
        repoLookupLoading={hub.repoLookupLoading}
        repoLookupSearched={hub.repoLookupSearched}
        repoFiles={hub.repoFiles}
        canCreate={hub.canCreate}
        createPending={hub.createModelPending}
        onSearchRepoFiles={() => void hub.searchRepoFiles()}
        onSelectRepoFile={hub.selectRepoFile}
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
