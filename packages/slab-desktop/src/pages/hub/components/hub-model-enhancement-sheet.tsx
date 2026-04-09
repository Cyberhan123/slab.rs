import {
  Loader2,
  Save,
  Settings2,
  Sparkles,
  TriangleAlert,
} from 'lucide-react';
import { useEffect, useMemo, useState, type PropsWithChildren } from 'react';
import { toast } from 'sonner';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@slab/components/sheet';

import type { ModelItem } from '../hooks/use-hub-model-catalog';
import {
  fetchModelEnhancement,
  type ModelEnhancementResponse,
  updateModelEnhancement,
} from '../lib/model-enhancement';

type HubModelEnhancementSheetProps = {
  model: ModelItem | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
};

type EnhancementFormState = {
  displayName: string;
  selectedPresetId: string;
  selectedVariantId: string;
  contextWindow: string;
  chatTemplate: string;
  temperature: string;
  topP: string;
};

const EMPTY_FORM: EnhancementFormState = {
  displayName: '',
  selectedPresetId: '',
  selectedVariantId: '',
  contextWindow: '',
  chatTemplate: '',
  temperature: '',
  topP: '',
};

export function HubModelEnhancementSheet({
  model,
  open,
  onOpenChange,
  onSaved,
}: HubModelEnhancementSheetProps) {
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [data, setData] = useState<ModelEnhancementResponse | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [form, setForm] = useState<EnhancementFormState>(EMPTY_FORM);

  useEffect(() => {
    if (!open || !model) {
      return;
    }

    let disposed = false;
    setIsLoading(true);
    setLoadError(null);

    void fetchModelEnhancement(model.id)
      .then((response) => {
        if (disposed) {
          return;
        }

        setData(response);
        setForm({
          displayName: response.model.display_name,
          selectedPresetId: response.selected_preset_id ?? '',
          selectedVariantId: response.selected_variant_id ?? '',
          contextWindow:
            response.model.spec.context_window !== undefined &&
            response.model.spec.context_window !== null
              ? String(response.model.spec.context_window)
              : '',
          chatTemplate: response.model.spec.chat_template ?? '',
          temperature:
            response.model.runtime_presets?.temperature !== undefined &&
            response.model.runtime_presets?.temperature !== null
              ? String(response.model.runtime_presets.temperature)
              : '',
          topP:
            response.model.runtime_presets?.top_p !== undefined &&
            response.model.runtime_presets?.top_p !== null
              ? String(response.model.runtime_presets.top_p)
              : '',
        });
      })
      .catch((error) => {
        if (disposed) {
          return;
        }

        setLoadError(error instanceof Error ? error.message : String(error));
        setData(null);
      })
      .finally(() => {
        if (!disposed) {
          setIsLoading(false);
        }
      });

    return () => {
      disposed = true;
    };
  }, [model, open]);

  const selectedPreset = useMemo(
    () => data?.presets.find((preset) => preset.id === form.selectedPresetId) ?? null,
    [data?.presets, form.selectedPresetId],
  );
  const selectedVariant = useMemo(
    () => data?.variants.find((variant) => variant.id === form.selectedVariantId) ?? null,
    [data?.variants, form.selectedVariantId],
  );
  const previewRepoId = selectedVariant?.repo_id ?? data?.resolved_spec.repo_id ?? '';
  const previewFilename = selectedVariant?.filename ?? data?.resolved_spec.filename ?? '';
  const previewLocalPath = selectedVariant?.local_path ?? data?.resolved_spec.local_path ?? '';
  const sourceWillChange = Boolean(
    data &&
      data.model.spec.local_path &&
      (normalizeText(data.model.spec.repo_id) !== normalizeText(previewRepoId) ||
        normalizeText(data.model.spec.filename) !== normalizeText(previewFilename)),
  );

  const canSave =
    !isLoading &&
    !isSaving &&
    Boolean(data) &&
    form.displayName.trim().length > 0;

  const handlePresetChange = (value: string) => {
    const presetId = value === '__none__' ? '' : value;
    const preset = data?.presets.find((item) => item.id === presetId);

    setForm((current) => ({
      ...current,
      selectedPresetId: presetId,
      selectedVariantId: preset?.variant_id ?? current.selectedVariantId,
    }));
  };

  const handleSave = async () => {
    if (!model || !data || !canSave) {
      return;
    }

    setIsSaving(true);
    try {
      await updateModelEnhancement(model.id, {
        display_name: form.displayName.trim(),
        selected_preset_id: emptyToNull(form.selectedPresetId),
        selected_variant_id: emptyToNull(form.selectedVariantId),
        context_window: parseOptionalInteger(form.contextWindow),
        chat_template: emptyToNull(form.chatTemplate),
        runtime_presets: {
          temperature: parseOptionalFloat(form.temperature),
          top_p: parseOptionalFloat(form.topP),
        },
      });

      toast.success('Model config updated.', {
        description: form.displayName.trim(),
      });
      onSaved();
      onOpenChange(false);
    } catch (error) {
      toast.error('Failed to update model config.', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="flex w-full max-w-[760px] flex-col gap-0 overflow-hidden border-l border-border/60 bg-[color:color-mix(in_oklab,var(--background)_92%,var(--surface-1))] p-0"
      >
        <SheetHeader className="shrink-0 border-b border-border/60 px-6 py-5 pr-14">
          <div className="flex items-start gap-3">
            <div className="flex size-11 items-center justify-center rounded-2xl bg-[var(--surface-soft)] text-[var(--brand-teal)]">
              <Settings2 className="size-5" />
            </div>
            <div className="space-y-1">
              <SheetTitle className="text-xl">Enhance model config</SheetTitle>
              <SheetDescription>
                Treat the imported pack as the source of truth, then choose the preset, variant,
                and runtime overrides you want to project into the catalog.
              </SheetDescription>
            </div>
          </div>
        </SheetHeader>

        <div className="flex-1 overflow-y-auto px-6 py-5">
          {isLoading ? (
            <div className="flex min-h-[260px] items-center justify-center text-muted-foreground">
              <Loader2 className="mr-2 size-4 animate-spin" />
              Loading model enhancement config...
            </div>
          ) : loadError ? (
            <Alert variant="destructive">
              <TriangleAlert className="size-4" />
              <AlertTitle>Failed to load enhancement config</AlertTitle>
              <AlertDescription>{loadError}</AlertDescription>
            </Alert>
          ) : data ? (
            <div className="space-y-6">
              <section className="grid gap-4 rounded-[28px] border border-border/60 bg-[var(--shell-card)]/55 p-5 md:grid-cols-2">
                <FieldBlock label="Display name">
                  <Input
                    value={form.displayName}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, displayName: event.target.value }))
                    }
                    placeholder="Qwen2.5-0.5B-Instruct"
                  />
                </FieldBlock>
                <FieldBlock label="Default preset">
                  <Select value={form.selectedPresetId || '__none__'} onValueChange={handlePresetChange}>
                    <SelectTrigger>
                      <SelectValue placeholder="Use pack default preset" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">Use pack default preset</SelectItem>
                      {data.presets.map((preset) => (
                        <SelectItem key={preset.id} value={preset.id}>
                          {preset.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FieldBlock>
                <FieldBlock label="Variant override">
                  <Select
                    value={form.selectedVariantId || '__none__'}
                    onValueChange={(value) =>
                      setForm((current) => ({
                        ...current,
                        selectedVariantId: value === '__none__' ? '' : value,
                      }))
                    }
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Follow preset/default variant" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">Follow preset/default variant</SelectItem>
                      {data.variants.map((variant) => (
                        <SelectItem key={variant.id} value={variant.id}>
                          {variant.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FieldBlock>
                <FieldBlock label="Context window">
                  <Input
                    inputMode="numeric"
                    value={form.contextWindow}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, contextWindow: event.target.value }))
                    }
                    placeholder="e.g. 32768"
                  />
                </FieldBlock>
                <FieldBlock label="Chat template" className="md:col-span-2">
                  <Input
                    value={form.chatTemplate}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, chatTemplate: event.target.value }))
                    }
                    placeholder="chatml"
                  />
                </FieldBlock>
                <FieldBlock label="Temperature">
                  <Input
                    inputMode="decimal"
                    value={form.temperature}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, temperature: event.target.value }))
                    }
                    placeholder="e.g. 0.7"
                  />
                </FieldBlock>
                <FieldBlock label="Top P">
                  <Input
                    inputMode="decimal"
                    value={form.topP}
                    onChange={(event) =>
                      setForm((current) => ({ ...current, topP: event.target.value }))
                    }
                    placeholder="e.g. 0.95"
                  />
                </FieldBlock>
              </section>

              <section className="space-y-3 rounded-[28px] border border-border/60 bg-[color:color-mix(in_oklab,var(--surface-1)_94%,var(--background))] p-5">
                <div className="flex items-center gap-2">
                  <Sparkles className="size-4 text-[var(--brand-gold)]" />
                  <h3 className="text-sm font-semibold tracking-[0.02em] text-foreground">
                    Resolved download target
                  </h3>
                </div>
                <div className="flex flex-wrap items-center gap-2">
                  {selectedPreset ? (
                    <Badge variant="chip" className="bg-[var(--surface-soft)]">
                      Preset: {selectedPreset.label}
                    </Badge>
                  ) : null}
                  {selectedVariant ? (
                    <Badge variant="chip" className="bg-[var(--surface-soft)]">
                      Variant: {selectedVariant.label}
                    </Badge>
                  ) : null}
                  {data.default_preset_id ? (
                    <Badge variant="chip" className="bg-[var(--surface-soft)]">
                      Pack default: {data.default_preset_id}
                    </Badge>
                  ) : null}
                </div>
                <PreviewRow label="Repo" value={previewRepoId || 'No remote repo for this source'} />
                <PreviewRow
                  label="File"
                  value={previewFilename || previewLocalPath || 'The selected pack source does not expose a single primary artifact'}
                />
                <PreviewRow
                  label="Runtime"
                  value={[
                    form.contextWindow.trim() ? `ctx=${form.contextWindow.trim()}` : null,
                    form.chatTemplate.trim() ? `template=${form.chatTemplate.trim()}` : null,
                    form.temperature.trim() ? `temp=${form.temperature.trim()}` : null,
                    form.topP.trim() ? `top_p=${form.topP.trim()}` : null,
                  ]
                    .filter(Boolean)
                    .join('  |  ') || 'Using pack/runtime defaults'}
                />
              </section>

              {sourceWillChange ? (
                <Alert>
                  <TriangleAlert className="size-4" />
                  <AlertTitle>Saving will detach the current local download</AlertTitle>
                  <AlertDescription>
                    The selected source no longer matches the file currently attached to this
                    catalog entry. After saving, the model will need to be downloaded again before
                    local loading.
                  </AlertDescription>
                </Alert>
              ) : null}
            </div>
          ) : null}
        </div>

        <div className="shrink-0 border-t border-border/60 px-6 py-4">
          <div className="flex items-center justify-between gap-3">
            <p className="text-xs leading-5 text-muted-foreground">
              Save writes the selected pack projection back into the catalog and preserves it in
              the `.slab` state for future startup syncs.
            </p>
            <div className="flex items-center gap-2">
              <Button
                variant="pill"
                onClick={() => onOpenChange(false)}
                disabled={isSaving}
              >
                Close
              </Button>
              <Button variant="cta" onClick={() => void handleSave()} disabled={!canSave}>
                {isSaving ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <Save className="size-4" />
                )}
                Save config
              </Button>
            </div>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}

function FieldBlock({
  label,
  className,
  children,
}: PropsWithChildren<{ label: string; className?: string }>) {
  return (
    <div className={cn('space-y-2', className)}>
      <Label className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </Label>
      {children}
    </div>
  );
}

function PreviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-1">
      <p className="text-[11px] font-semibold uppercase tracking-[0.16em] text-muted-foreground">
        {label}
      </p>
      <p className="break-all rounded-2xl border border-border/50 bg-[var(--shell-card)]/60 px-3 py-2 font-mono text-xs text-muted-foreground">
        {value}
      </p>
    </div>
  );
}

function emptyToNull(value: string) {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function normalizeText(value?: string | null) {
  return value?.trim() ?? '';
}

function parseOptionalInteger(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const parsed = Number.parseInt(trimmed, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

function parseOptionalFloat(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const parsed = Number.parseFloat(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function cn(...values: Array<string | undefined>) {
  return values.filter(Boolean).join(' ');
}
