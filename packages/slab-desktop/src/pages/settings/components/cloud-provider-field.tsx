import { useState } from 'react';
import { Controller, useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';
import { Cloud, Globe2, KeyRound, Pencil, Plus, Trash2 } from 'lucide-react';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@slab/components/dialog';
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@slab/components/field';
import { Input } from '@slab/components/input';
import { InputGroup, InputGroupAddon, InputGroupText } from '@slab/components/input-group';
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from '@slab/components/item';
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select';
import { useTranslation } from '@slab/i18n';

import {
  OPENAI_COMPATIBLE_VALUE,
  kindForFamily,
  kindsByGroup,
} from './cloud-provider-kinds';
import type { FieldErrorState, JsonObject, JsonValue } from '../types';

type CloudProviderFieldProps = {
  value: JsonValue;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
};

type RegistryEntry = {
  id: string;
  family: string;
  display_name: string;
  api_base: string;
  auth: { api_key: string | null; api_key_env: string | null };
};

const providerFormSchema = z.object({
  id: z
    .string()
    .min(1, 'ID is required')
    .regex(/^[a-z0-9][a-z0-9-_]*$/, 'Use lowercase letters, digits, - or _'),
  family: z.string().min(1, 'Select a provider family'),
  displayName: z.string().min(1, 'Display name is required'),
  apiBase: z
    .string()
    .min(1, 'API base URL is required')
    .regex(/^https?:\/\//i, 'Must start with http:// or https://'),
  apiKey: z.string(),
  apiKeyEnv: z.string(),
});

type ProviderFormValues = z.infer<typeof providerFormSchema>;

const EMPTY_FORM: ProviderFormValues = {
  id: '',
  family: OPENAI_COMPATIBLE_VALUE,
  displayName: '',
  apiBase: '',
  apiKey: '',
  apiKeyEnv: '',
};

export function CloudProviderField({ value, errorState, onChange }: CloudProviderFieldProps) {
  const { t } = useTranslation();
  const entries = coerceEntries(value);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);

  const form = useForm<ProviderFormValues>({
    resolver: zodResolver(providerFormSchema),
    defaultValues: EMPTY_FORM,
  });

  function openAdd() {
    setEditingIndex(null);
    form.reset(EMPTY_FORM);
    setDialogOpen(true);
  }

  function openEdit(index: number) {
    const entry = entries[index];
    if (!entry) return;
    setEditingIndex(index);
    form.reset({
      id: entry.id,
      family: entry.family || OPENAI_COMPATIBLE_VALUE,
      displayName: entry.display_name,
      apiBase: entry.api_base,
      apiKey: entry.auth.api_key ?? '',
      apiKeyEnv: entry.auth.api_key_env ?? '',
    });
    setDialogOpen(true);
  }

  function submit(values: ProviderFormValues) {
    const next: RegistryEntry = {
      id: values.id.trim(),
      family: values.family,
      display_name: values.displayName.trim(),
      api_base: values.apiBase.trim(),
      auth: {
        api_key: emptyToNull(values.apiKey),
        api_key_env: emptyToNull(values.apiKeyEnv),
      },
    };
    const updated =
      editingIndex === null
        ? [...entries, next]
        : entries.map((entry, index) => (index === editingIndex ? next : entry));
    onChange(updated.map(toJsonEntry));
    setDialogOpen(false);
  }

  function removeEntry(index: number) {
    onChange(entries.filter((_, entryIndex) => entryIndex !== index).map(toJsonEntry));
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-xs text-muted-foreground">
          {t('pages.settings.providerRegistry.configuredProviders', { count: entries.length })}
        </p>
        <Button variant="outline" size="sm" onClick={openAdd}>
          <Plus className="mr-2 h-4 w-4" />
          {t('pages.settings.providerRegistry.addProvider')}
        </Button>
      </div>

      {entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-6 text-sm text-muted-foreground">
          {t('pages.settings.providerRegistry.empty')}
        </div>
      ) : (
        <ItemGroup>
          {entries.map((entry, index) => {
            const kind = kindForFamily(entry.family);
            const title = entry.display_name.trim() || entry.id.trim() || kind.label;
            return (
              <Item key={entry.id || `provider-${index}`} className="pl-3">
                <ItemMedia>
                  <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-muted text-muted-foreground">
                    <Cloud className="h-4 w-4" />
                  </span>
                </ItemMedia>
                <ItemContent>
                  <ItemTitle className="flex flex-wrap items-center gap-2">
                    {title}
                    <Badge variant="outline" className="font-mono text-micro uppercase">
                      {kind.label}
                    </Badge>
                  </ItemTitle>
                  <ItemDescription className="font-mono text-xs">
                    {entry.api_base || t('pages.settings.providerRegistry.noApiBase')}
                  </ItemDescription>
                </ItemContent>
                <ItemActions>
                  <Button variant="ghost" size="icon-sm" onClick={() => openEdit(index)} aria-label="Edit provider">
                    <Pencil className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    onClick={() => removeEntry(index)}
                    aria-label="Remove provider"
                    className="text-destructive hover:text-destructive"
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </ItemActions>
              </Item>
            );
          })}
        </ItemGroup>
      )}

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>
              {editingIndex === null
                ? t('pages.settings.providerRegistry.dialog.addTitle')
                : t('pages.settings.providerRegistry.dialog.editTitle')}
            </DialogTitle>
            <DialogDescription>
              {t('pages.settings.providerRegistry.dialog.description')}
            </DialogDescription>
          </DialogHeader>

          <form id="cloud-provider-form" onSubmit={form.handleSubmit(submit)} className="space-y-4">
            <FieldGroup>
              <Controller
                control={form.control}
                name="family"
                render={({ field, fieldState }) => (
                  <Field>
                    <FieldLabel>{t('pages.settings.providerRegistry.fields.family.label')}</FieldLabel>
                    <Select value={field.value} onValueChange={(family) => applyFamilyDefaults(family, form)}>
                      <SelectTrigger className="h-11">
                        <SelectValue placeholder={t('pages.settings.providerRegistry.selectFamily')} />
                      </SelectTrigger>
                      <SelectContent>
                        {kindsByGroup().map(({ group, kinds }) => (
                          <SelectGroup key={group}>
                            <SelectLabel>{t(`pages.settings.providerRegistry.groups.${group}`)}</SelectLabel>
                            {kinds.map((kind) => (
                              <SelectItem key={kind.value} value={kind.value}>
                                {kind.label}
                              </SelectItem>
                            ))}
                          </SelectGroup>
                        ))}
                      </SelectContent>
                    </Select>
                    <FieldDescription>
                      {t('pages.settings.providerRegistry.fields.family.description')}
                    </FieldDescription>
                    {fieldState.error ? <FieldError>{fieldState.error.message}</FieldError> : null}
                  </Field>
                )}
              />

              <div className="grid gap-4 sm:grid-cols-2">
                <FormField
                  control={form.control}
                  name="id"
                  label={t('pages.settings.providerRegistry.fields.id.label')}
                  description={t('pages.settings.providerRegistry.fields.id.description')}
                  placeholder="openai-main"
                  error={form.formState.errors.id?.message}
                />
                <FormField
                  control={form.control}
                  name="displayName"
                  label={t('pages.settings.providerRegistry.fields.displayName.label')}
                  description={t('pages.settings.providerRegistry.fields.displayName.description')}
                  placeholder="OpenAI"
                  error={form.formState.errors.displayName?.message}
                />
              </div>

              <Controller
                control={form.control}
                name="apiBase"
                render={({ field, fieldState }) => (
                  <Field>
                    <FieldLabel>{t('pages.settings.providerRegistry.fields.apiBase.label')}</FieldLabel>
                    <InputGroup>
                      <InputGroupAddon>
                        <InputGroupText>
                          <Globe2 className="h-4 w-4" />
                        </InputGroupText>
                      </InputGroupAddon>
                      <input
                        {...field}
                        value={field.value ?? ''}
                        placeholder="https://api.openai.com/v1"
                        className="h-11 w-full rounded-lg border border-input bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                      />
                    </InputGroup>
                    <FieldDescription>
                      {t('pages.settings.providerRegistry.fields.apiBase.description')}
                    </FieldDescription>
                    {fieldState.error ? <FieldError>{fieldState.error.message}</FieldError> : null}
                  </Field>
                )}
              />

              <div className="grid gap-4 sm:grid-cols-2">
                <Controller
                  control={form.control}
                  name="apiKey"
                  render={({ field, fieldState }) => (
                    <Field>
                      <FieldLabel>{t('pages.settings.providerRegistry.fields.apiKey.label')}</FieldLabel>
                      <InputGroup>
                        <InputGroupAddon>
                          <InputGroupText>
                            <KeyRound className="h-4 w-4" />
                          </InputGroupText>
                        </InputGroupAddon>
                        <input
                          {...field}
                          value={field.value ?? ''}
                          type="password"
                          placeholder="sk-..."
                          className="h-11 w-full rounded-lg border border-input bg-transparent px-3 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        />
                      </InputGroup>
                      <FieldDescription>
                        {t('pages.settings.providerRegistry.fields.apiKey.description')}
                      </FieldDescription>
                      {fieldState.error ? <FieldError>{fieldState.error.message}</FieldError> : null}
                    </Field>
                  )}
                />
                <FormField
                  control={form.control}
                  name="apiKeyEnv"
                  label={t('pages.settings.providerRegistry.fields.apiKeyEnv.label')}
                  description={t('pages.settings.providerRegistry.fields.apiKeyEnv.description')}
                  placeholder="OPENAI_API_KEY"
                  error={form.formState.errors.apiKeyEnv?.message}
                />
              </div>
            </FieldGroup>
          </form>

          <DialogFooter>
            <DialogClose asChild>
              <Button type="button" variant="ghost">
                {t('pages.settings.providerRegistry.dialog.cancel')}
              </Button>
            </DialogClose>
            <Button type="submit" form="cloud-provider-form">
              {editingIndex === null
                ? t('pages.settings.providerRegistry.dialog.add')
                : t('pages.settings.providerRegistry.dialog.save')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {errorState?.path === '/' && errorState.message ? (
        <p className="text-sm text-destructive">{errorState.message}</p>
      ) : null}
    </div>
  );
}

function FormField({
  control,
  name,
  label,
  description,
  placeholder,
  error,
}: {
  control: ReturnType<typeof useForm<ProviderFormValues>>['control'];
  name: keyof ProviderFormValues;
  label: string;
  description: string;
  placeholder: string;
  error?: string;
}) {
  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Field>
          <FieldLabel>{label}</FieldLabel>
          <Input
            value={field.value ?? ''}
            onChange={field.onChange}
            onBlur={field.onBlur}
            name={field.name}
            placeholder={placeholder}
            className="h-11"
          />
          <FieldDescription>{description}</FieldDescription>
          {error ?? (fieldState.error ? <FieldError>{fieldState.error.message}</FieldError> : null)}
        </Field>
      )}
    />
  );
}

/** When the family changes, pre-fill display name / api base / key env from the kind defaults
 * (only when the user has not entered a value yet). */
function applyFamilyDefaults(
  family: string,
  form: ReturnType<typeof useForm<ProviderFormValues>>,
) {
  const kind = kindForFamily(family);
  form.setValue('family', family, { shouldValidate: true });
  if (!form.getValues('displayName')) {
    form.setValue('displayName', kind.value === OPENAI_COMPATIBLE_VALUE ? '' : kind.label);
  }
  if (!form.getValues('apiBase') && kind.defaultApiBase) {
    form.setValue('apiBase', kind.defaultApiBase);
  }
  if (!form.getValues('apiKeyEnv') && kind.defaultKeyEnv) {
    form.setValue('apiKeyEnv', kind.defaultKeyEnv);
  }
}

function coerceEntries(value: JsonValue): RegistryEntry[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((entry): entry is JsonObject => typeof entry === 'object' && entry !== null)
    .map((entry) => {
      const auth = readObject(entry.auth);
      return {
        id: readString(entry.id),
        family: readString(entry.family) || OPENAI_COMPATIBLE_VALUE,
        display_name: readString(entry.display_name),
        api_base: readString(entry.api_base),
        auth: {
          api_key: readNullableString(auth?.api_key),
          api_key_env: readNullableString(auth?.api_key_env),
        },
      };
    });
}

function toJsonEntry(entry: RegistryEntry): JsonValue {
  return {
    id: entry.id,
    family: entry.family,
    display_name: entry.display_name,
    api_base: entry.api_base,
    auth: {
      api_key: entry.auth.api_key,
      api_key_env: entry.auth.api_key_env,
    },
  } satisfies JsonObject;
}

function readObject(value: unknown): JsonObject | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as JsonObject) : null;
}
function readString(value: unknown): string {
  return typeof value === 'string' ? value : '';
}
function readNullableString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}
function emptyToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}
