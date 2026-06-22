import { useState } from 'react';
import { uniq } from 'lodash-es';
import { Globe2, KeyRound, Plus, Shield, Trash2 } from 'lucide-react';

import { Badge } from '@slab/components/badge';
import { Button } from '@slab/components/button';
import { Input } from '@slab/components/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@slab/components/select';
import { useTranslation } from '@slab/i18n';
import { cn } from '@/lib/utils';

import {
  jsonPointerAppend,
  pathContainsError,
  schemaFieldDescription,
  schemaFieldLabel,
  type JsonSchemaNode,
  type SettingsTranslate,
} from '../schema';
import type { FieldErrorState, JsonObject, JsonValue } from '../types';

type ProviderRegistryFieldProps = {
  schema: JsonSchemaNode;
  value: JsonValue;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
};

type StringMap = Record<string, string>;

type ProviderRegistryEntryDraft = {
  id: string;
  family: string;
  display_name: string;
  api_base: string;
  auth: {
    api_key: string | null;
    api_key_env: string | null;
  };
  defaults: {
    headers: StringMap;
    query: StringMap;
  };
};

const DEFAULT_PROVIDER_FAMILY = 'openai_compatible';

export function ProviderRegistryField({
  schema,
  value,
  errorState,
  onChange,
}: ProviderRegistryFieldProps) {
  const { t } = useTranslation();
  const entries = coerceProviderRegistryEntries(value);
  const familyOptions = providerFamilyOptions(schema);
  const entrySchema = schema.items;
  const entryProperties = entrySchema?.properties ?? {};
  const authProperties = entryProperties.auth?.properties ?? {};
  const defaultsProperties = entryProperties.defaults?.properties ?? {};
  const idSchema = entryProperties.id;
  const familySchema = entryProperties.family;
  const displayNameSchema = entryProperties.display_name;
  const apiBaseSchema = entryProperties.api_base;
  const apiKeySchema = authProperties.api_key;
  const apiKeyEnvSchema = authProperties.api_key_env;
  const headersSchema = defaultsProperties.headers;
  const querySchema = defaultsProperties.query;
  const registryTitle =
    schemaFieldLabel('', schema, t) || t('pages.settings.providerRegistry.title');

  function updateEntry(index: number, nextEntry: ProviderRegistryEntryDraft) {
    onChange(entries.map((entry, entryIndex) => (entryIndex === index ? toJsonEntry(nextEntry) : toJsonEntry(entry))));
  }

  function addEntry() {
    onChange([...entries.map(toJsonEntry), createEmptyEntry(familyOptions[0] ?? DEFAULT_PROVIDER_FAMILY)]);
  }

  function removeEntry(index: number) {
    onChange(entries.filter((_, entryIndex) => entryIndex !== index).map(toJsonEntry));
  }

  return (
    <div className="space-y-4 rounded-3xl border border-border/70 bg-muted/10 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-border/70 bg-background/70 px-4 py-3">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-muted-foreground" />
            <p className="text-sm font-medium">{registryTitle}</p>
          </div>
          <p className="text-xs text-muted-foreground">
            {t('pages.settings.providerRegistry.configuredProviders', {
              count: entries.length,
            })}
          </p>
        </div>

        <Button variant="outline" size="sm" onClick={addEntry}>
          <Plus className="mr-2 h-4 w-4" />
          {t('pages.settings.providerRegistry.addProvider')}
        </Button>
      </div>

      {entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-6 text-sm text-muted-foreground">
          {t('pages.settings.providerRegistry.empty')}
        </div>
      ) : null}

      <div className="space-y-4">
        {entries.map((entry, index) => {
          const entryPath = jsonPointerAppend('', index);
          const entryHasError = pathContainsError(entryPath, errorState?.path);
          const entryDescription = entrySchema
            ? schemaFieldDescription(entrySchema, t)
            : '';
          const title =
            entry.display_name.trim() ||
            entry.id.trim() ||
            t('pages.settings.providerRegistry.entryFallback', { index: index + 1 });

          return (
            <section
              key={entry.id}
              className={cn(
                'space-y-5 rounded-2xl border border-border/70 bg-background/85 p-5 shadow-elevation-2',
                entryHasError && 'border-destructive/60 bg-destructive/5',
              )}
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <h4 className="text-base font-semibold tracking-tight text-foreground">
                      {title}
                    </h4>
                    <Badge variant="outline" className="font-mono text-micro uppercase">
                      {entry.family}
                    </Badge>
                  </div>
                  <p className="text-xs leading-5 text-muted-foreground">
                    {entryDescription || t('pages.settings.providerRegistry.entryDescription')}
                  </p>
                  {errorState?.path === entryPath ? (
                    <p className="text-sm text-destructive">{errorState.message}</p>
                  ) : null}
                </div>

                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => removeEntry(index)}
                  className="text-destructive hover:text-destructive"
                >
                  <Trash2 className="mr-2 h-4 w-4" />
                  {t('pages.settings.providerRegistry.remove')}
                </Button>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <FieldBlock
                  label={schemaFieldLabel('id', idSchema ?? {}, t)}
                  description={
                    schemaFieldDescription(idSchema ?? {}, t) ||
                    t('pages.settings.providerRegistry.fields.id.description')
                  }
                  path={jsonPointerAppend(entryPath, 'id')}
                  errorState={errorState}
                >
                  <Input
                    value={entry.id}
                    onChange={(event) =>
                      updateEntry(index, { ...entry, id: event.target.value })
                    }
                    placeholder="openai-main"
                    className="h-11 rounded-2xl"
                  />
                </FieldBlock>

                <FieldBlock
                  label={schemaFieldLabel('display_name', displayNameSchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.displayName.description')}
                  path={jsonPointerAppend(entryPath, 'display_name')}
                  errorState={errorState}
                >
                  <Input
                    value={entry.display_name}
                    onChange={(event) =>
                      updateEntry(index, { ...entry, display_name: event.target.value })
                    }
                    placeholder="OpenAI"
                    className="h-11 rounded-2xl"
                  />
                </FieldBlock>

                <FieldBlock
                  label={schemaFieldLabel('family', familySchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.family.description')}
                  path={jsonPointerAppend(entryPath, 'family')}
                  errorState={errorState}
                >
                  <Select
                    value={entry.family}
                    onValueChange={(family) => updateEntry(index, { ...entry, family })}
                  >
                    <SelectTrigger className="h-11 rounded-2xl">
                      <SelectValue
                        placeholder={t('pages.settings.providerRegistry.selectFamily')}
                      />
                    </SelectTrigger>
                    <SelectContent>
                      {familyOptions.map((option) => (
                        <SelectItem key={option} value={option}>
                          {option}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FieldBlock>

                <FieldBlock
                  label={schemaFieldLabel('api_base', apiBaseSchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.apiBase.description')}
                  path={jsonPointerAppend(entryPath, 'api_base')}
                  errorState={errorState}
                >
                  <div className="relative">
                    <Globe2 className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      value={entry.api_base}
                      onChange={(event) =>
                        updateEntry(index, { ...entry, api_base: event.target.value })
                      }
                      placeholder="https://api.openai.com/v1"
                      className="h-11 rounded-2xl pl-10"
                    />
                  </div>
                </FieldBlock>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <FieldBlock
                  label={schemaFieldLabel('api_key', apiKeySchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.apiKey.description')}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'auth'), 'api_key')}
                  errorState={errorState}
                >
                  <div className="relative">
                    <KeyRound className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                    <Input
                      type="password"
                      value={entry.auth.api_key ?? ''}
                      onChange={(event) =>
                        updateEntry(index, {
                          ...entry,
                          auth: {
                            ...entry.auth,
                            api_key: emptyToNull(event.target.value),
                          },
                        })
                      }
                      placeholder="sk-live-..."
                      className="h-11 rounded-2xl pl-10"
                    />
                  </div>
                </FieldBlock>

                <FieldBlock
                  label={schemaFieldLabel('api_key_env', apiKeyEnvSchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.apiKeyEnv.description')}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'auth'), 'api_key_env')}
                  errorState={errorState}
                >
                  <Input
                    value={entry.auth.api_key_env ?? ''}
                    onChange={(event) =>
                      updateEntry(index, {
                        ...entry,
                        auth: {
                          ...entry.auth,
                          api_key_env: emptyToNull(event.target.value),
                        },
                      })
                    }
                    placeholder="OPENAI_API_KEY"
                    className="h-11 rounded-2xl"
                  />
                </FieldBlock>
              </div>

              <div className="grid gap-4 xl:grid-cols-2">
                <KeyValueMapEditor
                  label={schemaFieldLabel('headers', headersSchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.headers.description')}
                  value={entry.defaults.headers}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'defaults'), 'headers')}
                  t={t}
                  errorState={errorState}
                  onChange={(headers) =>
                    updateEntry(index, {
                      ...entry,
                      defaults: {
                        ...entry.defaults,
                        headers,
                      },
                    })
                  }
                />
                <KeyValueMapEditor
                  label={schemaFieldLabel('query', querySchema ?? {}, t)}
                  description={t('pages.settings.providerRegistry.fields.query.description')}
                  value={entry.defaults.query}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'defaults'), 'query')}
                  t={t}
                  errorState={errorState}
                  onChange={(query) =>
                    updateEntry(index, {
                      ...entry,
                      defaults: {
                        ...entry.defaults,
                        query,
                      },
                    })
                  }
                />
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}

function FieldBlock({
  label,
  description,
  path,
  errorState,
  children,
}: {
  label: string;
  description: string;
  path: string;
  errorState?: FieldErrorState;
  children: React.ReactNode;
}) {
  const fieldHasError = pathContainsError(path, errorState?.path);

  return (
    <div
      className={cn(
        'space-y-2 rounded-2xl border border-border/70 bg-background/70 p-4',
        fieldHasError && 'border-destructive/60 bg-destructive/5',
      )}
    >
      <div className="space-y-1">
        <p className="text-sm font-medium text-foreground">{label}</p>
        <p className="text-xs leading-5 text-muted-foreground">{description}</p>
      </div>
      {children}
      {errorState?.path === path ? (
        <p className="text-sm text-destructive">{errorState.message}</p>
      ) : null}
    </div>
  );
}

function KeyValueMapEditor({
  label,
  description,
  value,
  path,
  t,
  errorState,
  onChange,
}: {
  label: string;
  description: string;
  value: StringMap;
  path: string;
  t: SettingsTranslate;
  errorState?: FieldErrorState;
  onChange: (value: StringMap) => void;
}) {
  const [pendingKey, setPendingKey] = useState('');
  const [pendingValue, setPendingValue] = useState('');
  const entries = Object.entries(value);
  const fieldHasError = pathContainsError(path, errorState?.path);

  function updateValue(key: string, nextValue: string) {
    onChange({
      ...value,
      [key]: nextValue,
    });
  }

  function removeValue(key: string) {
    const next = { ...value };
    delete next[key];
    onChange(next);
  }

  function addPair() {
    const key = pendingKey.trim();
    if (!key) {
      return;
    }

    onChange({
      ...value,
      [key]: pendingValue,
    });
    setPendingKey('');
    setPendingValue('');
  }

  return (
    <div
      className={cn(
        'space-y-3 rounded-2xl border border-border/70 bg-background/70 p-4',
        fieldHasError && 'border-destructive/60 bg-destructive/5',
      )}
    >
      <div className="space-y-1">
        <p className="text-sm font-medium text-foreground">{label}</p>
        <p className="text-xs leading-5 text-muted-foreground">{description}</p>
      </div>

      {entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
          {t('pages.settings.providerRegistry.map.empty')}
        </div>
      ) : null}

      <div className="space-y-3">
        {entries.map(([key, entryValue]) => (
          <div key={key} className="grid gap-3 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_auto]">
            <Input value={key} readOnly className="h-11 rounded-2xl font-mono text-xs" />
            <Input
              value={entryValue}
              onChange={(event) => updateValue(key, event.target.value)}
              placeholder={t('pages.settings.providerRegistry.map.valuePlaceholder')}
              className="h-11 rounded-2xl"
            />
            <Button variant="ghost" size="sm" onClick={() => removeValue(key)}>
              <Trash2 className="mr-2 h-4 w-4" />
              {t('pages.settings.providerRegistry.remove')}
            </Button>
          </div>
        ))}
      </div>

      <div className="grid gap-3 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_auto]">
        <Input
          value={pendingKey}
          onChange={(event) => setPendingKey(event.target.value)}
          placeholder={t('pages.settings.providerRegistry.map.keyPlaceholder')}
          className="h-11 rounded-2xl font-mono text-xs"
        />
        <Input
          value={pendingValue}
          onChange={(event) => setPendingValue(event.target.value)}
          placeholder={t('pages.settings.providerRegistry.map.valuePlaceholder')}
          className="h-11 rounded-2xl"
        />
        <Button variant="outline" size="sm" onClick={addPair} disabled={pendingKey.trim().length === 0}>
          <Plus className="mr-2 h-4 w-4" />
          {t('pages.settings.providerRegistry.map.add')}
        </Button>
      </div>

      {errorState?.path === path ? (
        <p className="text-sm text-destructive">{errorState.message}</p>
      ) : null}
    </div>
  );
}

function providerFamilyOptions(schema: JsonSchemaNode): string[] {
  const familySchema = schema.items?.properties?.family;
  const values = Array.isArray(familySchema?.enum)
    ? familySchema.enum.filter((value): value is string => typeof value === 'string')
    : [];

  return values.length > 0 ? uniq(values) : [DEFAULT_PROVIDER_FAMILY];
}

function coerceProviderRegistryEntries(value: JsonValue): ProviderRegistryEntryDraft[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .filter((entry): entry is JsonObject => isJsonObject(entry))
    .map((entry) => ({
      id: readString(entry.id),
      family: readString(entry.family) || DEFAULT_PROVIDER_FAMILY,
      display_name: readString(entry.display_name),
      api_base: readString(entry.api_base),
      auth: {
        api_key: readNullableString(readObject(entry.auth)?.api_key),
        api_key_env: readNullableString(readObject(entry.auth)?.api_key_env),
      },
      defaults: {
        headers: readStringMap(readObject(readObject(entry.defaults)?.headers)),
        query: readStringMap(readObject(readObject(entry.defaults)?.query)),
      },
    }));
}

function createEmptyEntry(family: string): JsonValue {
  return toJsonEntry({
    id: '',
    family,
    display_name: '',
    api_base: '',
    auth: {
      api_key: null,
      api_key_env: null,
    },
    defaults: {
      headers: {},
      query: {},
    },
  });
}

function toJsonEntry(entry: ProviderRegistryEntryDraft): JsonValue {
  return {
    id: entry.id,
    family: entry.family,
    display_name: entry.display_name,
    api_base: entry.api_base,
    auth: {
      api_key: entry.auth.api_key,
      api_key_env: entry.auth.api_key_env,
    },
    defaults: {
      headers: entry.defaults.headers,
      query: entry.defaults.query,
    },
  } satisfies JsonObject;
}

function readObject(value: unknown): JsonObject | null {
  return isJsonObject(value) ? value : null;
}

function readString(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function readNullableString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function readStringMap(value: JsonObject | null): StringMap {
  if (!value) {
    return {};
  }

  return Object.fromEntries(
    Object.entries(value).filter((entry): entry is [string, string] => typeof entry[1] === 'string'),
  );
}

function emptyToNull(value: string): string | null {
  return value.length === 0 ? null : value;
}

function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
