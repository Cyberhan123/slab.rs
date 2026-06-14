import { useState } from 'react';
import { Plus, Trash2 } from 'lucide-react';

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
import { Switch } from '@slab/components/switch';
import { cn } from '@/lib/utils';

import {
  createDefaultJsonValue,
  isJsonObject,
  itemSummary,
  jsonPointerAppend,
  pathContainsError,
  schemaAllowsNull,
  schemaFieldLabel,
  schemaFieldPlaceholder,
  schemaPrimaryType,
  type JsonSchemaNode,
} from '../schema';
import type { FieldErrorState, JsonValue } from '../types';
import { parseSettingNumberValue } from '../utils';

type StructuredJsonFieldProps = {
  schema: JsonSchemaNode;
  value: JsonValue;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
};

type SchemaEditorProps = {
  schema: JsonSchemaNode;
  value: JsonValue;
  path: string;
  depth: number;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
};

export function StructuredJsonField({
  schema,
  value,
  errorState,
  onChange,
}: StructuredJsonFieldProps) {
  return (
    <div className="space-y-3 rounded-3xl border border-border/70 bg-muted/10 p-4">
      <SchemaNodeEditor
        schema={schema}
        value={value}
        path=""
        depth={0}
        errorState={errorState}
        onChange={onChange}
      />
      {errorState?.path === '/' ? (
        <p className="text-sm text-destructive">{errorState.message}</p>
      ) : null}
    </div>
  );
}

function SchemaNodeEditor({
  schema,
  value,
  path,
  depth,
  errorState,
  onChange,
}: SchemaEditorProps) {
  switch (schemaPrimaryType(schema)) {
    case 'array':
      return (
        <ArrayEditor
          schema={schema}
          value={value}
          path={path}
          depth={depth}
          errorState={errorState}
          onChange={onChange}
        />
      );
    case 'boolean':
      return <BooleanEditor value={value} onChange={onChange} />;
    case 'integer':
    case 'number':
      return <NumberEditor schema={schema} value={value} onChange={onChange} />;
    case 'object':
      return (
        <ObjectEditor
          schema={schema}
          value={value}
          path={path}
          depth={depth}
          errorState={errorState}
          onChange={onChange}
        />
      );
    case 'string':
    default:
      return <StringEditor schema={schema} value={value} onChange={onChange} />;
  }
}

function ObjectEditor({
  schema,
  value,
  path,
  depth,
  errorState,
  onChange,
}: SchemaEditorProps) {
  const objectValue = isJsonObject(value) ? value : {};
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {});
  const additionalSchema =
    schema.additionalProperties && typeof schema.additionalProperties === 'object'
      ? schema.additionalProperties
      : schema.additionalProperties === true
        ? ({ type: 'string' } satisfies JsonSchemaNode)
        : null;

  if (properties.length === 0 && !additionalSchema) {
    return (
      <div className="rounded-2xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
        No fields are defined for this object.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {properties.map(([key, childSchema]) => {
        const childPath = jsonPointerAppend(path, key);
        const childValue =
          key in objectValue ? objectValue[key] : createDefaultJsonValue(childSchema);
        const fieldHasError = pathContainsError(childPath, errorState?.path);

        return (
          <div
            key={childPath}
            className={cn(
              'space-y-3 rounded-2xl border border-border/70 bg-background/80 p-4',
              fieldHasError && 'border-destructive/60 bg-destructive/5',
              depth > 0 && 'shadow-[0_12px_30px_-28px_color-mix(in_oklab,var(--foreground)_32%,transparent)]',
            )}
          >
            <div className="space-y-1">
              <div className="flex flex-wrap items-center gap-2">
                <h4 className="text-sm font-medium">{schemaFieldLabel(key, childSchema)}</h4>
                {required.has(key) ? <Badge variant="secondary">Required</Badge> : null}
              </div>
              {childSchema.description ? (
                <p className="text-xs leading-5 text-muted-foreground">
                  {childSchema.description}
                </p>
              ) : null}
            </div>

            <SchemaNodeEditor
              schema={childSchema}
              value={childValue}
              path={childPath}
              depth={depth + 1}
              errorState={errorState}
              onChange={(nextValue) =>
                onChange({
                  ...objectValue,
                  [key]: nextValue,
                })
              }
            />

            {errorState?.path === childPath ? (
              <p className="text-sm text-destructive">{errorState.message}</p>
            ) : null}
          </div>
        );
      })}

      {additionalSchema ? (
        <AdditionalPropertiesEditor
          schema={additionalSchema}
          objectValue={objectValue}
          definedKeys={new Set(properties.map(([key]) => key))}
          path={path}
          depth={depth}
          errorState={errorState}
          onChange={onChange}
        />
      ) : null}
    </div>
  );
}

function AdditionalPropertiesEditor({
  schema,
  objectValue,
  definedKeys,
  path,
  depth,
  errorState,
  onChange,
}: {
  schema: JsonSchemaNode;
  objectValue: Record<string, JsonValue>;
  definedKeys: Set<string>;
  path: string;
  depth: number;
  errorState?: FieldErrorState;
  onChange: (value: JsonValue) => void;
}) {
  const [newKey, setNewKey] = useState('');
  const entries = Object.entries(objectValue).filter(([key]) => !definedKeys.has(key));
  const trimmedKey = newKey.trim();
  const canAdd =
    trimmedKey.length > 0 && !definedKeys.has(trimmedKey) && !(trimmedKey in objectValue);

  function addEntry() {
    if (!canAdd) {
      return;
    }

    onChange({
      ...objectValue,
      [trimmedKey]: createDefaultJsonValue(schema),
    });
    setNewKey('');
  }

  function removeEntry(key: string) {
    const nextValue = { ...objectValue };
    delete nextValue[key];
    onChange(nextValue);
  }

  return (
    <div className="space-y-3 rounded-2xl border border-dashed border-border/70 bg-background/60 p-4">
      <div className="flex flex-col gap-3 md:flex-row md:items-end">
        <div className="min-w-0 flex-1 space-y-1">
          <p className="text-sm font-medium">{schema.title ?? 'Additional properties'}</p>
          <p className="text-xs text-muted-foreground">{entries.length} configured</p>
        </div>
        <div className="flex min-w-0 flex-1 gap-2">
          <Input
            value={newKey}
            onChange={(event) => setNewKey(event.target.value)}
            placeholder="Property name"
            className="h-10 min-w-0 rounded-2xl"
          />
          <Button variant="outline" size="sm" onClick={addEntry} disabled={!canAdd}>
            <Plus className="mr-2 h-4 w-4" />
            Add
          </Button>
        </div>
      </div>

      {entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
          No entries configured yet.
        </div>
      ) : null}

      {entries.map(([key, entryValue]) => (
        <AdditionalPropertyEntry
          key={jsonPointerAppend(path, key)}
          schema={schema}
          propertyKey={key}
          value={entryValue}
          path={jsonPointerAppend(path, key)}
          depth={depth}
          errorState={errorState}
          onRemove={() => removeEntry(key)}
          onChange={(nextValue) =>
            onChange({
              ...objectValue,
              [key]: nextValue,
            })
          }
        />
      ))}
    </div>
  );
}

function AdditionalPropertyEntry({
  schema,
  propertyKey,
  value,
  path,
  depth,
  errorState,
  onRemove,
  onChange,
}: {
  schema: JsonSchemaNode;
  propertyKey: string;
  value: JsonValue;
  path: string;
  depth: number;
  errorState?: FieldErrorState;
  onRemove: () => void;
  onChange: (value: JsonValue) => void;
}) {
  const hasError = pathContainsError(path, errorState?.path);

  return (
    <div
      className={cn(
        'space-y-3 rounded-2xl border border-border/70 bg-background/80 p-4',
        hasError && 'border-destructive/60 bg-destructive/5',
      )}
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Badge variant="outline">{propertyKey}</Badge>
        <Button variant="ghost" size="sm" onClick={onRemove}>
          <Trash2 className="mr-2 h-4 w-4" />
          Remove
        </Button>
      </div>

      <SchemaNodeEditor
        schema={schema}
        value={value}
        path={path}
        depth={depth + 1}
        errorState={errorState}
        onChange={onChange}
      />

      {errorState?.path === path ? (
        <p className="text-sm text-destructive">{errorState.message}</p>
      ) : null}
    </div>
  );
}

function ArrayEditor({
  schema,
  value,
  path,
  depth,
  errorState,
  onChange,
}: SchemaEditorProps) {
  const itemSchema = schema.items;
  const items = Array.isArray(value) ? value : [];
  const minItems = Math.max(0, schema.minItems ?? 0);

  if (!itemSchema) {
    return (
      <div className="rounded-2xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
        This list does not describe its item shape yet.
      </div>
    );
  }

  const resolvedItemSchema = itemSchema;

  function addItem() {
    onChange([...items, createDefaultJsonValue(resolvedItemSchema)]);
  }

  function updateItem(index: number, nextValue: JsonValue) {
    onChange(items.map((item, itemIndex) => (itemIndex === index ? nextValue : item)));
  }

  function removeItem(index: number) {
    onChange(items.filter((_, itemIndex) => itemIndex !== index));
  }

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-border/70 bg-background/70 px-4 py-3">
        <div className="space-y-1">
          <p className="text-sm font-medium">{schema.title ?? 'Items'}</p>
          <p className="text-xs text-muted-foreground">
            {items.length} configured
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={addItem}>
          <Plus className="mr-2 h-4 w-4" />
          Add {resolvedItemSchema.title ?? 'item'}
        </Button>
      </div>

      {items.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-border/70 px-4 py-4 text-sm text-muted-foreground">
          No entries configured yet.
        </div>
      ) : null}

      {items.map((item, index) => {
        const itemPath = jsonPointerAppend(path, index);
        const itemHasError = pathContainsError(itemPath, errorState?.path);
        const summary = itemSummary(item);

        return (
          <div
            key={itemPath}
            className={cn(
              'space-y-4 rounded-2xl border border-border/70 bg-background/80 p-4',
              itemHasError && 'border-destructive/60 bg-destructive/5',
              depth > 0 && 'shadow-[0_16px_40px_-34px_color-mix(in_oklab,var(--foreground)_32%,transparent)]',
            )}
          >
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="space-y-1">
                <div className="flex flex-wrap items-center gap-2">
                  <h4 className="text-sm font-medium">
                    {resolvedItemSchema.title ?? 'Item'} {index + 1}
                  </h4>
                  {summary ? <Badge variant="outline">{summary}</Badge> : null}
                </div>
                {resolvedItemSchema.description ? (
                  <p className="text-xs leading-5 text-muted-foreground">
                    {resolvedItemSchema.description}
                  </p>
                ) : null}
              </div>

              <Button
                variant="ghost"
                size="sm"
                onClick={() => removeItem(index)}
                disabled={items.length <= minItems}
              >
                <Trash2 className="mr-2 h-4 w-4" />
                Remove
              </Button>
            </div>

            <SchemaNodeEditor
              schema={resolvedItemSchema}
              value={item}
              path={itemPath}
              depth={depth + 1}
              errorState={errorState}
              onChange={(nextValue) => updateItem(index, nextValue)}
            />

            {errorState?.path === itemPath ? (
              <p className="text-sm text-destructive">{errorState.message}</p>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function StringEditor({
  schema,
  value,
  onChange,
}: Pick<SchemaEditorProps, 'schema' | 'value' | 'onChange'>) {
  const currentValue = typeof value === 'string' ? value : '';
  const placeholder = schemaFieldPlaceholder(schema);

  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return (
      <Select value={currentValue} onValueChange={(nextValue) => onChange(nextValue)}>
        <SelectTrigger className="h-11 w-full rounded-2xl">
          <SelectValue placeholder={placeholder ?? 'Select an option'} />
        </SelectTrigger>
        <SelectContent>
          {schema.enum.map((option) => (
            <SelectItem key={option} value={option}>
              {option}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  return (
    <Input
      type={schema.writeOnly ? 'password' : 'text'}
      value={currentValue}
      onChange={(event) => {
        const nextValue = event.target.value;
        if (nextValue.trim().length === 0 && schemaAllowsNull(schema)) {
          onChange(null);
          return;
        }

        onChange(nextValue);
      }}
      placeholder={placeholder}
      className="h-11 rounded-2xl"
    />
  );
}

function NumberEditor({
  schema,
  value,
  onChange,
}: Pick<SchemaEditorProps, 'schema' | 'value' | 'onChange'>) {
  const numberType = schemaPrimaryType(schema) === 'integer' ? 'integer' : 'number';
  const currentValue = typeof value === 'number' ? String(value) : '';

  return (
    <Input
      inputMode="numeric"
      value={currentValue}
      onChange={(event) => {
        const trimmed = event.target.value.trim();
        if (!trimmed) {
          onChange(schemaAllowsNull(schema) ? null : 0);
          return;
        }

        const nextValue = parseSettingNumberValue(trimmed, numberType);

        if (nextValue !== null) {
          onChange(nextValue);
        }
      }}
      placeholder={schemaFieldPlaceholder(schema)}
      className="h-11 rounded-2xl"
    />
  );
}

function BooleanEditor({
  value,
  onChange,
}: Pick<SchemaEditorProps, 'value' | 'onChange'>) {
  return (
    <div className="flex items-center justify-between rounded-2xl border border-border/70 bg-background/70 px-4 py-3">
      <span className="text-sm text-muted-foreground">
        {value === true ? 'Enabled' : 'Disabled'}
      </span>
      <Switch checked={value === true} onCheckedChange={(nextValue) => onChange(nextValue)} />
    </div>
  );
}
