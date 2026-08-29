import { Loader2, LockKeyhole, RotateCw, Save, Settings2, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { toast } from 'sonner';
import { translateServerField, useTranslation } from '@slab/i18n';
import { getErrorMessage } from '@slab/api';

import { Alert, AlertDescription, AlertTitle } from '@slab/components/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@slab/components/alert-dialog';
import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { Input } from '@slab/components/input';
import { Label } from '@slab/components/label';
import { Switch } from '@slab/components/switch';
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
  type UpdateModelConfigSelectionRequest,
  getModelConfigField,
  useModelConfigDocumentQuery,
  useUpdateModelConfigSelectionMutation,
} from '@slab/core/models/config';

// Whitelisted config-field paths the user may override after import. Must match
// the server-side whitelist in `validate_load_overrides` / the editable set in
// `build_model_config_sections`. chat_template / gbnf stay locked (asset-ref vs.
// raw-source gap, deferred).
const LOAD_OVERRIDE_PATHS = [
  'load.num_workers',
  'load.context_length',
  'load.diffusion_model_path',
  'load.vae_path',
  'load.taesd_path',
  'load.clip_l_path',
  'load.clip_g_path',
  'load.t5xxl_path',
  'load.flash_attn',
  'load.offload_params_to_cpu',
  'load.vae_device',
  'load.clip_device',
] as const;

const INFERENCE_OVERRIDE_PATHS = ['inference.temperature', 'inference.top_p'] as const;

type HubModelEnhancementSheetProps = {
  model: ModelItem | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
  onReload: (model: ModelItem) => void;
};

export function HubModelEnhancementSheet({
  model,
  open,
  onOpenChange,
  onSaved,
  onReload,
}: HubModelEnhancementSheetProps) {
  const { t } = useTranslation();
  const [selectedPresetId, setSelectedPresetId] = useState('');
  const [selectedVariantId, setSelectedVariantId] = useState('');
  const [loadOverrides, setLoadOverrides] = useState<Record<string, unknown>>({});
  const [inferenceOverrides, setInferenceOverrides] = useState<Record<string, unknown>>({});
  const [reloadConfirmOpen, setReloadConfirmOpen] = useState(false);
  const {
    data,
    error,
    isLoading,
  } = useModelConfigDocumentQuery(open && model ? model.id : null, {
    enabled: open && Boolean(model),
  });
  const updateModelConfigSelectionMutation = useUpdateModelConfigSelectionMutation();
  const loadError = error ? getErrorMessage(error) : null;
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
      setLoadOverrides({});
      setInferenceOverrides({});
      setReloadConfirmOpen(false);
    }
  }, [model, open]);

  const dirtyLoadScope = useMemo(
    () =>
      LOAD_OVERRIDE_PATHS.some((path) => {
        if (!data) return false;
        const field = getModelConfigField(data, path);
        const key = overrideKeyFromPath(path);
        if (!field || !key) return false;
        return key in loadOverrides && loadOverrides[key] !== field.effective_value;
      }),
    [data, loadOverrides],
  );

  const dirtyInferenceScope = useMemo(
    () =>
      INFERENCE_OVERRIDE_PATHS.some((path) => {
        if (!data) return false;
        const field = getModelConfigField(data, path);
        const key = overrideKeyFromPath(path);
        if (!field || !key) return false;
        return key in inferenceOverrides && inferenceOverrides[key] !== field.effective_value;
      }),
    [data, inferenceOverrides],
  );

  const savePayload = useMemo<UpdateModelConfigSelectionRequest | null>(() => {
    if (!data) {
      return null;
    }
    const selection = buildSelectionPayload(data, selectedPresetId, selectedVariantId);
    if (!selection) {
      return null;
    }
    const payload: UpdateModelConfigSelectionRequest = {
      selected_preset_id: selection.selected_preset_id,
      selected_variant_id: selection.selected_variant_id,
    };
    if (dirtyLoadScope) {
      payload.load_overrides = buildOverridePayload(data, LOAD_OVERRIDE_PATHS, loadOverrides);
    }
    if (dirtyInferenceScope) {
      payload.inference_overrides = buildOverridePayload(
        data,
        INFERENCE_OVERRIDE_PATHS,
        inferenceOverrides,
      );
    }
    return payload;
  }, [data, selectedPresetId, selectedVariantId, loadOverrides, inferenceOverrides, dirtyLoadScope, dirtyInferenceScope]);

  const selectionDirty =
    (savePayload?.selected_preset_id ?? null) !== (data?.selection.selected_preset_id ?? null) ||
    (savePayload?.selected_variant_id ?? null) !== (data?.selection.selected_variant_id ?? null);

  const canSave =
    Boolean(data) &&
    !isLoading &&
    !isSaving &&
    Boolean(savePayload) &&
    (selectionDirty || dirtyLoadScope || dirtyInferenceScope);

  const handlePresetChange = (value: string) => {
    const nextPreset =
      data?.selection.presets.find((preset) => preset.id === value) ?? null;
    setSelectedPresetId(value);
    setSelectedVariantId(nextPreset?.variant_id ?? data?.selection.default_variant_id ?? '');
    // Switching preset re-derives load/inference config from the pack; drop any
    // local override edits so they don't get pinned onto the new base.
    setLoadOverrides({});
    setInferenceOverrides({});
  };

  const handleVariantChange = (value: string) => {
    setSelectedVariantId(value);
    setLoadOverrides({});
    setInferenceOverrides({});
  };

  const handleFieldOverrideChange = (path: string, value: unknown) => {
    const key = overrideKeyFromPath(path);
    if (!key) {
      return;
    }
    const setter = path.startsWith('load.') ? setLoadOverrides : setInferenceOverrides;
    setter((prev) => ({ ...prev, [key]: value }));
  };

  const handleFieldOverrideReset = (path: string) => {
    const key = overrideKeyFromPath(path);
    if (!key) {
      return;
    }
    const setter = path.startsWith('load.') ? setLoadOverrides : setInferenceOverrides;
    setter((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
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
      // Load-scoped edits only take effect after a reload; ask before doing it
      // so an in-flight inference isn't interrupted. Inference edits are live.
      if (dirtyLoadScope && model.runtime_state?.loaded) {
        setReloadConfirmOpen(true);
      } else {
        onOpenChange(false);
      }
    } catch (err) {
      toast.error(t('pages.hub.toast.selectionUpdateFailed'), {
        description: err instanceof Error ? err.message : String(err),
      });
    }
  };

  const handleConfirmReload = () => {
    setReloadConfirmOpen(false);
    if (model) {
      onReload(model);
    }
    onOpenChange(false);
  };

  const handleSkipReload = () => {
    setReloadConfirmOpen(false);
    onOpenChange(false);
  };

  return (
    <>
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent
          side="right"
          className="flex w-full max-w-[780px] flex-col gap-0 overflow-hidden border-l border-border/60 bg-[color:color-mix(in_oklab,var(--background)_92%,var(--card))] p-0"
        >
          <SheetHeader className="shrink-0 border-b border-border/60 px-6 py-5 pr-14">
            <div className="flex items-start gap-3">
              <div className="flex size-11 items-center justify-center rounded-2xl bg-secondary text-primary">
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

                <section className="grid gap-4 rounded-3xl border border-border/60 bg-glass-bg p-5 md:grid-cols-2">
                  <ReadOnlyBlock
                    label={t('pages.hub.sheet.blocks.displayName')}
                    value={data.model_summary.display_name}
                  />
                  <ReadOnlyBlock
                    label={t('common.fields.backend')}
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
                    <Select value={selectedVariantId} onValueChange={handleVariantChange}>
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

                {data.sections.map((section) => {
                  const sectionLabel = translateServerField(section.i18n, 'label', section.label, t);
                  const sectionDescription = translateServerField(
                    section.i18n,
                    'description_md',
                    section.description_md,
                    t,
                  );
                  return (
                    <section
                      key={section.id}
                      className="space-y-4 rounded-3xl border border-border/60 bg-glass-bg p-5"
                    >
                      <div className="space-y-1">
                        <h3 className="text-base font-semibold tracking-tight text-foreground">
                          {sectionLabel}
                        </h3>
                        {sectionDescription ? (
                          <p className="text-xs leading-5 text-muted-foreground">
                            {sectionDescription}
                          </p>
                        ) : null}
                      </div>

                      <div className="space-y-3">
                        {section.fields.map((field) => {
                          const overrideKey = overrideKeyFromPath(field.path);
                          const overridesMap = field.path.startsWith('load.')
                            ? loadOverrides
                            : inferenceOverrides;
                          const hasOverride = overrideKey !== null && overrideKey in overridesMap;
                          return (
                            <FieldCard
                              key={field.path}
                              field={field}
                              hasOverride={hasOverride}
                              overrideValue={
                                hasOverride && overrideKey !== null
                                  ? overridesMap[overrideKey]
                                  : undefined
                              }
                              onChange={(value) => handleFieldOverrideChange(field.path, value)}
                              onReset={() => handleFieldOverrideReset(field.path)}
                            />
                          );
                        })}
                      </div>
                    </section>
                  );
                })}
              </div>
            ) : null}
          </div>

          <div className="flex shrink-0 items-center justify-end gap-3 border-t border-border/60 px-6 py-4">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.actions.close')}
            </Button>
            <Button onClick={handleSave} disabled={!canSave}>
              {isSaving ? <Loader2 className="mr-2 size-4 animate-spin" /> : <Save className="mr-2 size-4" />}
              {t('pages.hub.sheet.blocks.saveSelection')}
            </Button>
          </div>
        </SheetContent>
      </Sheet>

      <AlertDialog open={reloadConfirmOpen} onOpenChange={setReloadConfirmOpen}>
        <AlertDialogContent data-testid="hub-model-reload-confirm">
          <AlertDialogHeader>
            <AlertDialogTitle>{t('pages.hub.sheet.reload.title')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('pages.hub.sheet.reload.description')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={handleSkipReload}>
              {t('common.actions.later')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                handleConfirmReload();
              }}
            >
              <RotateCw className="mr-2 size-4" />
              {t('common.actions.reload')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function ReadOnlyBlock({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-2">
      <Label className="text-xs font-semibold uppercase tracking-eyebrow text-muted-foreground">
        {label}
      </Label>
      <div className="rounded-[14px] border border-border/60 bg-secondary px-4 py-3 text-sm font-medium text-foreground">
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
      <Label className="text-xs font-semibold uppercase tracking-eyebrow text-muted-foreground">
        {label}
      </Label>
      {children}
    </div>
  );
}

function FieldCard({
  field,
  hasOverride,
  overrideValue,
  onChange,
  onReset,
}: {
  field: ModelConfigFieldResponse;
  hasOverride: boolean;
  overrideValue: unknown;
  onChange: (value: unknown) => void;
  onReset: () => void;
}) {
  const { t } = useTranslation();
  const fieldLabel = translateServerField(field.i18n, 'label', field.label, t);
  const fieldDescription = translateServerField(
    field.i18n,
    'description_md',
    field.description_md,
    t,
  );
  const displayValue = hasOverride ? overrideValue : field.effective_value;
  const isEdited = hasOverride && overrideValue !== field.effective_value;

  return (
    <div className="rounded-[20px] border border-border/60 bg-background/70 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="text-sm font-semibold tracking-tight text-foreground">
              {fieldLabel}
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
            {isEdited ? (
              <Badge variant="outline" className="rounded-full text-primary">
                {t('pages.hub.sheet.blocks.edited')}
              </Badge>
            ) : null}
          </div>
          {fieldDescription ? (
            <p className="text-xs leading-5 text-muted-foreground">{fieldDescription}</p>
          ) : null}
          <p className="text-caption uppercase tracking-eyebrow text-muted-foreground">
            {field.path}
          </p>
        </div>
        {field.editable && isEdited ? (
          <Button variant="ghost" size="sm" onClick={onReset}>
            {t('pages.hub.sheet.blocks.resetToPack')}
          </Button>
        ) : null}
      </div>

      <div className="mt-4">
        {field.editable
          ? renderEditableControl(field, displayValue, onChange, onReset, t)
          : renderFieldValue(field, displayValue, t)}
      </div>
    </div>
  );
}

function renderEditableControl(
  field: ModelConfigFieldResponse,
  value: unknown,
  onChange: (value: unknown) => void,
  onReset: () => void,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  switch (field.value_type) {
    case 'boolean':
      return (
        <div className="flex items-center gap-3 rounded-[14px] border border-border/60 bg-secondary px-4 py-3">
          <Switch checked={Boolean(value)} onCheckedChange={(checked) => onChange(checked)} />
          <span className="text-sm font-medium text-foreground">
            {value
              ? t('common.status.enabled')
              : t('common.status.disabled')}
          </span>
        </div>
      );
    case 'integer':
    case 'number':
      return (
        <Input
          type="number"
          value={value === null || value === undefined ? '' : String(value)}
          onChange={(event) => {
            const raw = event.target.value;
            if (raw.trim() === '') {
              onReset();
              return;
            }
            const parsed =
              field.value_type === 'integer' ? parseInt(raw, 10) : parseFloat(raw);
            onChange(Number.isNaN(parsed) ? null : parsed);
          }}
        />
      );
    case 'path':
    case 'string':
      return (
        <Input
          value={typeof value === 'string' ? value : ''}
          onChange={(event) => {
            const raw = event.target.value;
            if (raw.trim() === '') {
              onReset();
              return;
            }
            onChange(raw);
          }}
        />
      );
    case 'json':
    default:
      // JSON fields stay read-only for now (structured editor deferred).
      return renderFieldValue(field, value, t);
  }
}

function renderFieldValue(
  field: ModelConfigFieldResponse,
  value: unknown,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  if (value === null || value === undefined) {
    return (
      <div className="rounded-[14px] border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
        {t('pages.hub.sheet.blocks.notSet')}
      </div>
    );
  }

  if (field.value_type === 'boolean') {
    return (
      <div className="rounded-[14px] border border-border/60 bg-secondary px-4 py-3 text-sm font-medium text-foreground">
        {value
          ? t('common.status.enabled')
          : t('common.status.disabled')}
      </div>
    );
  }

  if (field.value_type === 'json' || typeof value === 'object') {
    return (
      <pre className="overflow-x-auto rounded-[14px] border border-border/60 bg-secondary px-4 py-3 text-xs leading-6 text-foreground">
        {JSON.stringify(value, null, 2)}
      </pre>
    );
  }

  return (
    <div className="rounded-[14px] border border-border/60 bg-secondary px-4 py-3 text-sm font-medium text-foreground">
      {String(value)}
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

function overrideKeyFromPath(path: string): string | null {
  if (path.startsWith('load.')) return path.slice('load.'.length);
  if (path.startsWith('inference.')) return path.slice('inference.'.length);
  return null;
}

/// Builds the overrides map to persist for a scope. Emits every whitelisted
/// field's effective value (overridden value where the user edited, otherwise the
/// pack-derived value), so the server-side "replace" semantics preserve prior
/// overrides for fields the user didn't touch this round.
function buildOverridePayload(
  data: ModelConfigDocumentResponse,
  paths: readonly string[],
  overrides: Record<string, unknown>,
): Record<string, unknown> {
  const payload: Record<string, unknown> = {};
  for (const path of paths) {
    const field = getModelConfigField(data, path);
    const key = overrideKeyFromPath(path);
    if (!field || !key) {
      continue;
    }
    const value = key in overrides ? overrides[key] : field.effective_value;
    if (value !== null && value !== undefined) {
      payload[key] = value;
    }
  }
  return payload;
}
