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
      <DialogContent className="max-h-[calc(100dvh-2rem)] max-w-3xl gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b border-border/60 px-5 pt-5 pb-4">
          <DialogTitle>Add model entry</DialogTitle>
          <DialogDescription>
            Add a model from a Hugging Face repo. You can type the filename manually or
            pick it from search results.
          </DialogDescription>
        </DialogHeader>

        <div className="overflow-y-auto px-5 py-4">
          <div className="space-y-5">
            <div className="grid gap-4 md:grid-cols-2">
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
                <Label htmlFor="hub-filename">Filename</Label>
                <Input
                  id="hub-filename"
                  value={form.filename}
                  onChange={(event) => setField('filename', event.target.value)}
                  placeholder="Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
                />
              </div>
            </div>

            <div className="grid gap-2">
              <Label htmlFor="hub-repo-id">Hugging Face repo</Label>
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
                  Search
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                Search the repo to pick a filename, or leave it as a manual entry.
              </p>
            </div>

            {(repoLookupRepoId || repoLookupLoading || repoLookupSearched) ? (
              <div className="space-y-3 rounded-2xl border border-border/70 bg-muted/10 p-4">
                {repoLookupRepoId ? (
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div className="min-w-0">
                      <p className="truncate text-sm font-medium">{repoLookupRepoId}</p>
                      <p className="text-xs text-muted-foreground">
                        Showing up to 200 files.
                      </p>
                    </div>
                    <div className="relative w-full sm:w-64">
                      <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                      <Input
                        value={repoLookupFilter}
                        onChange={(event) => setRepoLookupFilter(event.target.value)}
                        placeholder="Filter files"
                        className="pl-9"
                      />
                    </div>
                  </div>
                ) : null}

                {form.filename.trim() ? (
                  <div className="rounded-xl border border-border/70 bg-background px-3 py-2">
                    <p className="text-xs text-muted-foreground">Selected file</p>
                    <p className="break-all font-mono text-xs">{form.filename}</p>
                  </div>
                ) : null}

                <div className="max-h-56 space-y-2 overflow-y-auto pr-1">
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
                        No files match the current filter.
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
                  ) : null}
                </div>
              </div>
            ) : null}

            <div className="grid gap-2">
              <Label>Backends</Label>
              <div className="grid gap-2 sm:grid-cols-3">
                {BACKEND_OPTIONS.map((backend) => (
                  <label
                    key={backend.id}
                    className={cn(
                      'flex cursor-pointer items-start gap-3 rounded-xl border px-3 py-3 transition-colors',
                      form.backendIds.includes(backend.id)
                        ? 'border-primary bg-primary/5'
                        : 'border-border/70 bg-background hover:bg-muted/30',
                    )}
                  >
                    <Checkbox
                      checked={form.backendIds.includes(backend.id)}
                      onCheckedChange={(checked) => toggleBackend(backend.id, checked === true)}
                    />
                    <div className="min-w-0">
                      <p className="text-sm font-medium">{backend.label}</p>
                      <p className="text-xs text-muted-foreground">{backend.description}</p>
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
        </div>

        <DialogFooter showCloseButton className="border-t border-border/60 px-5 py-4">
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
