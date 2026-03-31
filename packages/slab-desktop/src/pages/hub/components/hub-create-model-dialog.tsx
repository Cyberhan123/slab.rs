import { FileJson, Loader2, Plus, Upload } from 'lucide-react';

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

type HubCreateModelDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selectedFileName: string | null;
  setCreateFile: (file: File | null) => void;
  canCreate: boolean;
  createPending: boolean;
  onCreate: () => void;
};

const MODEL_CONFIG_EXAMPLE = `{
  "id": "qwen2_5_0_5b_instruct_q4_k_m",
  "display_name": "Qwen2.5 0.5B Instruct",
  "provider": "local.ggml.llama",
  "spec": {
    "repo_id": "bartowski/Qwen2.5-0.5B-Instruct-GGUF",
    "filename": "Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"
  }
}`;

export function HubCreateModelDialog({
  open,
  onOpenChange,
  selectedFileName,
  setCreateFile,
  canCreate,
  createPending,
  onCreate,
}: HubCreateModelDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] max-w-3xl gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b border-border/60 px-5 pt-5 pb-4">
          <DialogTitle>Import model config</DialogTitle>
          <DialogDescription>
            Upload a model JSON config. The server stores it under its model config directory and
            reloads it during startup.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 overflow-y-auto px-5 py-4">
          <div className="space-y-3 rounded-2xl border border-border/70 bg-muted/10 p-4">
            <div className="grid gap-2">
              <Label htmlFor="hub-model-config-file">Model config JSON</Label>
              <Input
                id="hub-model-config-file"
                type="file"
                accept=".json,application/json"
                onChange={(event) => setCreateFile(event.target.files?.[0] ?? null)}
                disabled={createPending}
              />
            </div>

            {selectedFileName ? (
              <div className="rounded-xl border border-border/70 bg-background px-3 py-3">
                <div className="flex items-center gap-2 text-sm font-medium">
                  <FileJson className="h-4 w-4 text-primary" />
                  <span className="truncate">{selectedFileName}</span>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  This file will be validated, stored, and turned into a catalog entry.
                </p>
              </div>
            ) : (
              <div className="rounded-xl border border-dashed border-border/70 bg-background px-4 py-6 text-center text-sm text-muted-foreground">
                <Upload className="mx-auto mb-3 h-5 w-5" />
                Choose a JSON config file to import a model entry.
              </div>
            )}
          </div>

          <div className="space-y-2 rounded-2xl border border-border/70 bg-background p-4">
            <p className="text-sm font-medium">Expected shape</p>
            <p className="text-xs text-muted-foreground">
              At minimum include <code>id</code>, <code>display_name</code>, <code>provider</code>,
              and a provider-specific <code>spec</code> object.
            </p>
            <pre className="overflow-x-auto rounded-xl border border-border/70 bg-muted/30 p-3 text-xs leading-5">
              <code>{MODEL_CONFIG_EXAMPLE}</code>
            </pre>
          </div>
        </div>

        <DialogFooter showCloseButton className="border-t border-border/60 px-5 py-4">
          <Button onClick={onCreate} disabled={!canCreate}>
            {createPending ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <Plus className="mr-2 h-4 w-4" />
            )}
            Import config
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
