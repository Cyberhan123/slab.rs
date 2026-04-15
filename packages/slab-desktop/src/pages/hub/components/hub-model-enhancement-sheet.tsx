import { Loader2, LockKeyhole, Save, Settings2, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { toast } from 'sonner';
import { useTranslation } from '@slab/i18n';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
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
  type ModelConfigDocumentResponse,
  type ModelConfigFieldResponse,
  useModelConfigDocumentQuery,
  useUpdateModelConfigSelectionMutation,
} from '@/lib/model-config';

type HubModelEnhancementSheetProps = {
  model: ModelItem | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
};

export function HubModelEnhancementSheet({
  model,
  open,
  onOpenChange,
  onSaved,
}: HubModelEnhancementSheetProps) {
  const { t } = useTranslation();
  const [selectedPresetId, setSelectedPresetId] = useState('');
  const [selectedVariantId, setSelectedVariantId] = useState('');
  const {
    data,
    error,
    isLoading,
  } = useModelConfigDocumentQuery(open && model ? model.id : null, {
    enabled: open && Boolean(model),
  });
  const updateModelConfigSelectionMutation = useUpdateModelConfigSelectionMutation();
  const loadError = error instanceof Error ? error.message : error ? String(error) : null;
  const isSaving = updateModelConfigSelectionMutation.isPending;

  useEffect(() => {
    if (!open || !model || !data) {
      return;
    }

    setSelectedPresetId(
      data.selection.effective_preset_id ??
        data.selection.selected_preset_id ??
        data.selection.default_preset_id ??
        '',
    );
    setSelectedVariantId(
      data.selection.effective_variant_id ??
        data.selection.selected_variant_id ??
        data.selection.default_variant_id ??
        '',
    );
  }, [data, model, open]);

  useEffect(() => {
    if (!open || !model) {
      setSelectedPresetId('');
      setSelectedVariantId('');
    }
  }, [model, open]);

  const savePayload = useMemo(
    () => buildSelectionPayload(data ?? null, selectedPresetId, selectedVariantId),
    [data, selectedPresetId, selectedVariantId],
  );

  const canSave =
    Boolean(data) &&
    !isLoading &&
    !isSaving &&
    Boolean(savePayload) &&
    (savePayload?.selected_preset_id !== (data?.selection.selected_preset_id ?? null) ||
      savePayload?.selected_variant_id !== (data?.selection.selected_variant_id ?? null));

  const handlePresetChange = (value: string) => {
    const nextPreset =
      data?.selection.presets.find((preset) => preset.id === value) ?? null;
    setSelectedPresetId(value);
    setSelectedVariantId(nextPreset?.variant_id ?? data?.selection.default_variant_id ?? '');
  };

  const handleSave = async () => {
    if (!model || !savePayload) {
      return;
    }

    try {
      await updateModelConfigSelectionMutation.mutateAsync({
        params: {
          path: { id: model.id },
        },
        body: savePayload,
      });
      toast.success(t('pages.hub.toast.selectionUpdated'), {
        description: data?.model_summary.display_name ?? model.display_name,
      });
      onSaved();
      onOpenChange(false);
    } catch (error) {
      toast.error(t('pages.hub.toast.selectionUpdateFailed'), {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="flex w-full max-w-[780px] flex-col gap-0 overflow-hidden border-l border-border/60 bg-[color:color-mix(in_oklab,var(--background)_92%,var(--surface-1))] p-0"
      >
        <SheetHeader className="shrink-0 border-b border-border/60 px-6 py-5 pr-14">
          <div className="flex items-start gap-3">
            <div className="flex size-11 items-center justify-center rounded-2xl bg-[var(--surface-soft)] text-[var(--brand-teal)]">
              <Settings2 className="size-5" />
            </div>
            <div className="space-y-1">
              <SheetTitle className="text-xl">{t('pages.hub.sheet.title')}</SheetTitle>
              <SheetDescription>
                {t('pages.hub.sheet.description')}
              </SheetDescription>
            </div>
          </div>
        </SheetHeader>

        <div className="flex-1 overflow-y-auto px-6 py-5">
          {isLoading ? (
            <div className="flex min-h-[260px] items-center justify-center text-muted-foreground">
              <Loader2 className="mr-2 size-4 animate-spin" />
              {t('pages.hub.sheet.loading')}
            </div>
          ) : loadError ? (
            <Alert variant="destructive">
              <TriangleAlert className="size-4" />
              <AlertTitle>{t('pages.hub.sheet.failedLoadTitle')}</AlertTitle>
              <AlertDescription>{loadError}</AlertDescription>
            </Alert>
          ) : data ? (
            <div className="space-y-6">
              {data.warnings.length > 0 ? (
                <Alert>
                  <TriangleAlert className="size-4" />
                  <AlertTitle>{t('pages.hub.sheet.selectionWarningTitle')}</AlertTitle>
                  <AlertDescription>{data.warnings.join(' ')}</AlertDescription>
                </Alert>
              ) : null}

              <section className="grid gap-4 rounded-[28px] border border-border/60 bg-[var(--shell-card)]/55 p-5 md:grid-cols-2">
                <ReadOnlyBlock
                  label={t('pages.hub.sheet.blocks.displayName')}
                  value={data.model_summary.display_name}
                />
                <ReadOnlyBlock
                  label={t('pages.hub.sheet.blocks.backend')}
                  value={data.model_summary.backend_id ?? data.model_summary.kind}
                />
                <FieldBlock label={t('pages.hub.sheet.blocks.preset')}>
                  <Select value={selectedPresetId} onValueChange={handlePresetChange}>
                    <SelectTrigger>
                      <SelectValue placeholder={t('pages.hub.sheet.blocks.presetPlaceholder')} />
                    </SelectTrigger>
                    <SelectContent>
                      {data.selection.presets.map((preset) => (
                        <SelectItem key={preset.id} value={preset.id}>
                          {preset.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FieldBlock>
                <FieldBlock label={t('pages.hub.sheet.blocks.variant')}>
                  <Select value={selectedVariantId} onValueChange={setSelectedVariantId}>
                    <SelectTrigger>
                      <SelectValue placeholder={t('pages.hub.sheet.blocks.variantPlaceholder')} />
                    </SelectTrigger>
                    <SelectContent>
                      {data.selection.variants.map((variant) => (
                        <SelectItem key={variant.id} value={variant.id}>
                          {variant.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FieldBlock>
              </section>

              {data.sections.map((section) => (
                <section
                  key={section.id}
                  className="space-y-4 rounded-[28px] border border-border/60 bg-[var(--shell-card)]/55 p-5"
                >
                  <div className="space-y-1">
                    <h3 className="text-base font-semibold tracking-[-0.02em] text-foreground">
                      {section.label}
                    </h3>
                    {section.description_md ? (
                      <p className="text-xs leading-5 text-muted-foreground">
                        {section.description_md}
                      </p>
                    ) : null}
                  </div>

                  <div className="space-y-3">
                    {section.fields.map((field) => (
                      <ReadonlyFieldCard key={field.path} field={field} />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center justify-end gap-3 border-t border-border/60 px-6 py-4">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('pages.hub.sheet.blocks.close')}
          </Button>
          <Button onClick={handleSave} disabled={!canSave}>
            {isSaving ? <Loader2 className="mr-2 size-4 animate-spin" /> : <Save className="mr-2 size-4" />}
            {t('pages.hub.sheet.blocks.saveSelection')}
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  );
}

function ReadOnlyBlock({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-2">
      <Label className="text-xs font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {label}
      </Label>
      <div className="rounded-[14px] border border-border/60 bg-[var(--surface-soft)] px-4 py-3 text-sm font-medium text-foreground">
        {value}
      </div>
    </div>
  );
}

function FieldBlock({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="space-y-2">
      <Label className="text-xs font-semibold uppercase tracking-[0.08em] text-muted-foreground">
        {label}
      </Label>
      {children}
    </div>
  );
}

function ReadonlyFieldCard({ field }: { field: ModelConfigFieldResponse }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-[20px] border border-border/60 bg-background/70 p-4 shadow-[0_1px_2px_color-mix(in_oklab,var(--foreground)_8%,transparent)]">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="text-sm font-semibold tracking-[-0.02em] text-foreground">
              {field.label}
            </h4>
              <Badge variant="secondary" className="rounded-full">
                {formatOrigin(field.origin, t)}
              </Badge>
              {field.locked ? (
                <Badge variant="outline" className="rounded-full">
                  <LockKeyhole className="mr-1 size-3" />
                  {t('pages.hub.sheet.blocks.packLocked')}
                </Badge>
              ) : null}
          </div>
          {field.description_md ? (
            <p className="text-xs leading-5 text-muted-foreground">{field.description_md}</p>
          ) : null}
          <p className="text-[11px] uppercase tracking-[0.08em] text-muted-foreground">
            {field.path}
          </p>
        </div>
      </div>

      <div className="mt-4">{renderFieldValue(field, t)}</div>
    </div>
  );
}

function renderFieldValue(
  field: ModelConfigFieldResponse,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  if (field.effective_value === null || field.effective_value === undefined) {
    return (
      <div className="rounded-[14px] border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
        {t('pages.hub.sheet.blocks.notSet')}
      </div>
    );
  }

  if (field.value_type === 'boolean') {
    return (
      <div className="rounded-[14px] border border-border/60 bg-[var(--surface-soft)] px-4 py-3 text-sm font-medium text-foreground">
        {field.effective_value
          ? t('pages.hub.sheet.blocks.enabled')
          : t('pages.hub.sheet.blocks.disabled')}
      </div>
    );
  }

  if (field.value_type === 'json' || typeof field.effective_value === 'object') {
    return (
      <pre className="overflow-x-auto rounded-[14px] border border-border/60 bg-[var(--surface-soft)] px-4 py-3 text-xs leading-6 text-foreground">
        {JSON.stringify(field.effective_value, null, 2)}
      </pre>
    );
  }

  return (
    <div className="rounded-[14px] border border-border/60 bg-[var(--surface-soft)] px-4 py-3 text-sm font-medium text-foreground">
      {String(field.effective_value)}
    </div>
  );
}

function formatOrigin(
  origin: ModelConfigFieldResponse['origin'],
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  switch (origin) {
    case 'pack_manifest':
      return t('pages.hub.sheet.blocks.origin.pack_manifest');
    case 'selected_preset':
      return t('pages.hub.sheet.blocks.origin.selected_preset');
    case 'selected_variant':
      return t('pages.hub.sheet.blocks.origin.selected_variant');
    case 'selected_backend_config':
      return t('pages.hub.sheet.blocks.origin.selected_backend_config');
    case 'pmid_fallback':
      return t('pages.hub.sheet.blocks.origin.pmid_fallback');
    case 'derived':
      return t('pages.hub.sheet.blocks.origin.derived');
    default:
      return origin;
  }
}

function buildSelectionPayload(
  data: ModelConfigDocumentResponse | null,
  presetId: string,
  variantId: string,
) {
  if (!data) {
    return null;
  }

  const preset =
    data.selection.presets.find((candidate) => candidate.id === presetId) ?? null;
  const defaultVariantId = preset?.variant_id ?? data.selection.default_variant_id ?? null;

  return {
    selected_preset_id:
      presetId && presetId !== data.selection.default_preset_id ? presetId : null,
    selected_variant_id:
      variantId && variantId !== defaultVariantId ? variantId : null,
  };
}
