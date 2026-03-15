import { Loader2, Plus, Search } from 'lucide-react';

import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

import {
  BACKEND_OPTIONS,
  type CreateForm,
} from '../hooks/use-hub-model-catalog';

type HubCreateModelDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  form: CreateForm;
  setField: <K extends keyof CreateForm>(key: K, value: CreateForm[K]) => void;
  toggleBackend: (id: string, checked: boolean) => void;
  repoLookupRepoId?: string;
  repoLookupFilter: string;
  setRepoLookupFilter: (value: string) => void;
  repoLookupLoading: boolean;
  repoLookupSearched: boolean;
  repoFiles: string[];
  canCreate: boolean;
  createPending: boolean;
  onSearchRepoFiles: () => void;
  onSelectRepoFile: (file: string) => void;
  onCreate: () => void;
};

export function HubCreateModelDialog({
  open,
  onOpenChange,
  form,
  setField,
  toggleBackend,
  repoLookupRepoId,
  repoLookupFilter,
  setRepoLookupFilter,
  repoLookupLoading,
  repoLookupSearched,
  repoFiles,
  canCreate,
  createPending,
  onSearchRepoFiles,
  onSelectRepoFile,
  onCreate,
}: HubCreateModelDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-5xl">
        <DialogHeader>
          <DialogTitle>Add model entry</DialogTitle>
          <DialogDescription>
            Save a new catalog entry. Repo lookup uses <code>/v1/models/available</code>{' '}
            so you can inspect files before creating the model.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-5 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
          <div className="space-y-5 rounded-2xl border border-border/70 bg-background/70 p-5">
            <div className="space-y-2">
              <p className="text-sm font-medium">Entry details</p>
              <p className="text-sm text-muted-foreground">
                Fill the catalog metadata first, then use repo lookup on the right to pick
                the exact file.
              </p>
            </div>

            <div className="grid gap-2">
              <Label htmlFor="hub-display-name">Display name</Label>
              <Input
                id="hub-display-name"
                value={form.displayName}
                onChange={(event) => setField('displayName', event.target.value)}
                placeholder="Qwen2.5 0.5B Instruct"
              />
            </div>

            <div className="grid gap-2">
              <Label htmlFor="hub-filename">Filename inside repo</Label>
              <Input
                id="hub-filename"
                value={form.filename}
                onChange={(event) => setField('filename', event.target.value)}
                placeholder="Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
              />
              <p className="text-xs text-muted-foreground">
                You can type this manually or pick it from repository lookup.
              </p>
            </div>

            <div className="grid gap-3">
              <Label>Backends</Label>
              <div className="grid gap-3">
                {BACKEND_OPTIONS.map((backend) => (
                  <label
                    key={backend.id}
                    className="flex cursor-pointer gap-3 rounded-2xl border border-border/70 bg-background/80 p-4"
                  >
                    <Checkbox
                      checked={form.backendIds.includes(backend.id)}
                      onCheckedChange={(checked) => toggleBackend(backend.id, checked === true)}
                    />
                    <div className="space-y-1">
                      <p className="font-medium">{backend.label}</p>
                      <p className="text-sm text-muted-foreground">{backend.description}</p>
                    </div>
                  </label>
                ))}
              </div>
              {form.backendIds.length === 0 ? (
                <p className="text-xs text-destructive">
                  Select at least one backend before creating the entry.
                </p>
              ) : null}
            </div>
          </div>

          <div className="space-y-5 rounded-2xl border border-border/70 bg-muted/10 p-5">
            <div className="space-y-2">
              <p className="text-sm font-medium">Repository lookup</p>
              <p className="text-sm text-muted-foreground">
                1. Enter a Hugging Face repo ID. 2. Search files. 3. Click a file to copy
                it into the form.
              </p>
            </div>

            <div className="grid gap-2">
              <Label htmlFor="hub-repo-id">Hugging Face repo ID</Label>
              <div className="flex flex-col gap-2 sm:flex-row">
                <Input
                  id="hub-repo-id"
                  value={form.repoId}
                  onChange={(event) => setField('repoId', event.target.value)}
                  placeholder="bartowski/Qwen2.5-0.5B-Instruct-GGUF"
                />
                <Button
                  type="button"
                  variant="outline"
                  onClick={onSearchRepoFiles}
                  disabled={repoLookupLoading || !form.repoId.trim()}
                >
                  {repoLookupLoading ? (
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  ) : (
                    <Search className="mr-2 h-4 w-4" />
                  )}
                  Search repo
                </Button>
              </div>
            </div>

            {repoLookupRepoId ? (
              <div className="grid gap-3">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <div>
                    <p className="text-sm font-medium">{repoLookupRepoId}</p>
                    <p className="text-xs text-muted-foreground">
                      Showing up to 200 files from this repository.
                    </p>
                  </div>
                  <div className="relative w-full sm:w-72">
                    <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      value={repoLookupFilter}
                      onChange={(event) => setRepoLookupFilter(event.target.value)}
                      placeholder="Filter repo files"
                      className="pl-9"
                    />
                  </div>
                </div>

                {form.filename.trim() ? (
                  <div className="rounded-xl border border-border/70 bg-background/80 px-3 py-2">
                    <p className="text-xs text-muted-foreground">Selected file</p>
                    <p className="break-all font-mono text-xs">{form.filename}</p>
                  </div>
                ) : null}
              </div>
            ) : null}

            <div className="max-h-72 space-y-2 overflow-y-auto pr-1">
              {repoLookupRepoId ? (
                repoFiles.length > 0 ? (
                  repoFiles.map((file) => (
                    <button
                      key={file}
                      type="button"
                      onClick={() => onSelectRepoFile(file)}
                      className={cn(
                        'w-full rounded-xl border px-3 py-2 text-left transition-colors',
                        form.filename.trim() === file
                          ? 'border-primary bg-primary/5'
                          : 'border-border/70 bg-background hover:bg-muted/40',
                      )}
                    >
                      <p className="break-all font-mono text-xs">{file}</p>
                    </button>
                  ))
                ) : (
                  <p className="text-sm text-muted-foreground">
                    No repo files match the current filter.
                  </p>
                )
              ) : repoLookupLoading ? (
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Searching repo files...
                </div>
              ) : repoLookupSearched ? (
                <p className="text-sm text-muted-foreground">
                  No files were returned for this repo.
                </p>
              ) : (
                <p className="text-sm text-muted-foreground">
                  Search a repo ID to browse its files.
                </p>
              )}
            </div>
          </div>
        </div>

        <DialogFooter showCloseButton>
          <Button onClick={onCreate} disabled={!canCreate}>
            {createPending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Plus className="mr-2 h-4 w-4" />
            )}
            Create entry
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
