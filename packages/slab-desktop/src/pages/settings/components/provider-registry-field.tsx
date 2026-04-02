import { useState } from 'react';
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
import { cn } from '@/lib/utils';

import {
  jsonPointerAppend,
  pathContainsError,
  type JsonSchemaNode,
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
  const entries = coerceProviderRegistryEntries(value);
  const familyOptions = providerFamilyOptions(schema);

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
            <p className="text-sm font-medium">Provider Registry</p>
          </div>
          <p className="text-xs text-muted-foreground">
            {entries.length} configured provider{entries.length === 1 ? '' : 's'}
          </p>
        </div>

        <Button variant="outline" size="sm" onClick={addEntry}>
          <Plus className="mr-2 h-4 w-4" />
          Add provider
        </Button>
      </div>

      {entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-6 text-sm text-muted-foreground">
          No providers configured yet.
        </div>
      ) : null}

      <div className="space-y-4">
        {entries.map((entry, index) => {
          const entryPath = jsonPointerAppend('', index);
          const entryHasError = pathContainsError(entryPath, errorState?.path);
          const title = entry.display_name.trim() || entry.id.trim() || `Provider ${index + 1}`;

          return (
            <section
              key={`${entry.id}-${index}`}
              className={cn(
                'space-y-5 rounded-[24px] border border-border/70 bg-background/85 p-5 shadow-[0_16px_40px_-34px_color-mix(in_oklab,var(--foreground)_28%,transparent)]',
                entryHasError && 'border-destructive/60 bg-destructive/5',
              )}
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="space-y-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <h4 className="text-base font-semibold tracking-[-0.02em] text-foreground">
                      {title}
                    </h4>
                    <Badge variant="outline" className="font-mono text-[10px] uppercase">
                      {entry.family}
                    </Badge>
                  </div>
                  <p className="text-xs leading-5 text-muted-foreground">
                    Configure connection details and optional request defaults for one remote provider.
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
                  Remove
                </Button>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <FieldBlock
                  label="Provider ID"
                  description="Stable internal identifier used by the app."
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
                  label="Display name"
                  description="Friendly label shown in the UI."
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
                  label="Family"
                  description="Protocol family used by this provider."
                  path={jsonPointerAppend(entryPath, 'family')}
                  errorState={errorState}
                >
                  <Select
                    value={entry.family}
                    onValueChange={(family) => updateEntry(index, { ...entry, family })}
                  >
                    <SelectTrigger className="h-11 rounded-2xl">
                      <SelectValue placeholder="Select a provider family" />
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
                  label="API base URL"
                  description="Base URL for the provider endpoint."
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
                  label="Literal API key"
                  description="Optional secret stored directly in settings."
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
                  label="API key env var"
                  description="Environment variable name used when no literal key is stored."
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
                  label="Default headers"
                  description="Optional headers added to every request sent through this provider."
                  value={entry.defaults.headers}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'defaults'), 'headers')}
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
                  label="Default query params"
                  description="Optional query parameters added to every request."
                  value={entry.defaults.query}
                  path={jsonPointerAppend(jsonPointerAppend(entryPath, 'defaults'), 'query')}
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
  errorState,
  onChange,
}: {
  label: string;
  description: string;
  value: StringMap;
  path: string;
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
          No entries configured.
        </div>
      ) : null}

      <div className="space-y-3">
        {entries.map(([key, entryValue]) => (
          <div key={key} className="grid gap-3 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_auto]">
            <Input value={key} readOnly className="h-11 rounded-2xl font-mono text-xs" />
            <Input
              value={entryValue}
              onChange={(event) => updateValue(key, event.target.value)}
              placeholder="Value"
              className="h-11 rounded-2xl"
            />
            <Button variant="ghost" size="sm" onClick={() => removeValue(key)}>
              <Trash2 className="mr-2 h-4 w-4" />
              Remove
            </Button>
          </div>
        ))}
      </div>

      <div className="grid gap-3 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.2fr)_auto]">
        <Input
          value={pendingKey}
          onChange={(event) => setPendingKey(event.target.value)}
          placeholder="Key"
          className="h-11 rounded-2xl font-mono text-xs"
        />
        <Input
          value={pendingValue}
          onChange={(event) => setPendingValue(event.target.value)}
          placeholder="Value"
          className="h-11 rounded-2xl"
        />
        <Button variant="outline" size="sm" onClick={addPair} disabled={pendingKey.trim().length === 0}>
          <Plus className="mr-2 h-4 w-4" />
          Add
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

  return values.length > 0 ? Array.from(new Set(values)) : [DEFAULT_PROVIDER_FAMILY];
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