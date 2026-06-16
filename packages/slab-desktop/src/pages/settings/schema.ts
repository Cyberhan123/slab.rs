import { clamp, compact } from 'lodash-es';

import { translateServerField, type ServerI18nPayload } from '@slab/i18n';

import type { JsonObject, JsonValue, SettingResponse } from './types';

type JsonSchemaType =
  | 'array'
  | 'boolean'
  | 'integer'
  | 'null'
  | 'number'
  | 'object'
  | 'string';

export type JsonSchemaNode = {
  'x-i18n'?: ServerI18nPayload;
  additionalProperties?: boolean | JsonSchemaNode;
  default?: JsonValue;
  description?: string;
  enum?: string[];
  examples?: JsonValue[];
  format?: string;
  items?: JsonSchemaNode;
  maxItems?: number;
  maxLength?: number;
  minItems?: number;
  minLength?: number;
  pattern?: string;
  properties?: Record<string, JsonSchemaNode>;
  required?: string[];
  title?: string;
  type?: JsonSchemaType | JsonSchemaType[];
  writeOnly?: boolean;
};

export type SettingsTranslate = Parameters<typeof translateServerField>[3];

export function parseStructuredJsonSchema(
  property: SettingResponse,
): JsonSchemaNode | null {
  if (property.schema.type !== 'array' && property.schema.type !== 'object') {
    return null;
  }

  const rawSchema = property.schema.json_schema;
  if (!isJsonSchemaNode(rawSchema)) {
    return null;
  }

  return schemaPrimaryType(rawSchema) === property.schema.type ? rawSchema : null;
}

export function schemaPrimaryType(schema: JsonSchemaNode): JsonSchemaType | null {
  if (Array.isArray(schema.type)) {
    return schema.type.find((candidate) => candidate !== 'null') ?? null;
  }

  return schema.type ?? null;
}

export function schemaAllowsNull(schema: JsonSchemaNode): boolean {
  return Array.isArray(schema.type) && schema.type.includes('null');
}

export function createDefaultJsonValue(schema: JsonSchemaNode): JsonValue {
  if (isJsonValue(schema.default)) {
    return cloneJsonValue(schema.default);
  }

  switch (schemaPrimaryType(schema)) {
    case 'array': {
      const minItems = clamp(schema.minItems ?? 0, 0, Number.POSITIVE_INFINITY);
      const itemSchema = schema.items;

      if (!itemSchema) {
        return [];
      }

      return Array.from({ length: minItems }, () => createDefaultJsonValue(itemSchema));
    }
    case 'boolean':
      return false;
    case 'integer':
    case 'number':
      return 0;
    case 'object': {
      const next: JsonObject = {};
      for (const [key, childSchema] of Object.entries(schema.properties ?? {})) {
        next[key] = createDefaultJsonValue(childSchema);
      }
      return next;
    }
    case 'string':
    default:
      return '';
  }
}

export function cloneJsonValue(value: JsonValue): JsonValue {
  if (Array.isArray(value)) {
    return value.map((item) => cloneJsonValue(item));
  }

  if (isJsonObject(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, itemValue]) => [key, cloneJsonValue(itemValue)]),
    );
  }

  return value;
}

export function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function jsonPointerAppend(path: string, segment: string | number): string {
  const encoded = String(segment).replaceAll('~', '~0').replaceAll('/', '~1');
  return path ? `${path}/${encoded}` : `/${encoded}`;
}

export function pathContainsError(targetPath: string, errorPath?: string): boolean {
  if (!errorPath) {
    return false;
  }

  return errorPath === targetPath || errorPath.startsWith(`${targetPath}/`);
}

export function schemaFieldLabel(
  key: string,
  schema: JsonSchemaNode,
  t?: SettingsTranslate,
): string {
  const fallback = schema.title ?? humanizeIdentifier(key);
  return t ? translateServerField(schema['x-i18n'], 'title', fallback, t) : fallback;
}

export function schemaFieldDescription(
  schema: JsonSchemaNode,
  t?: SettingsTranslate,
): string {
  return t
    ? translateServerField(schema['x-i18n'], 'description', schema.description, t)
    : (schema.description ?? '');
}

export function schemaFieldPlaceholder(
  schema: JsonSchemaNode,
  t?: SettingsTranslate,
): string | undefined {
  const example = schema.examples?.find((value): value is string => typeof value === 'string');
  if (example) {
    return example;
  }

  const label = schemaFieldLabel('', schema, t);
  if (!label) {
    return undefined;
  }

  return t
    ? t('pages.settings.field.enterNamedValue', { label })
    : `Enter ${label.toLowerCase()}`;
}

export function itemSummary(value: JsonValue): string | null {
  if (!isJsonObject(value)) {
    return null;
  }

  for (const key of ['name', 'display_name', 'id', 'remote_model']) {
    const candidate = value[key];
    if (typeof candidate === 'string' && candidate.trim().length > 0) {
      return candidate.trim();
    }
  }

  return null;
}

function isJsonSchemaNode(value: unknown): value is JsonSchemaNode {
  return isJsonObject(value);
}

function isJsonValue(value: unknown): value is JsonValue {
  if (
    value === null ||
    typeof value === 'boolean' ||
    typeof value === 'number' ||
    typeof value === 'string'
  ) {
    return true;
  }

  if (Array.isArray(value)) {
    return value.every((item) => isJsonValue(item));
  }

  if (isJsonObject(value)) {
    return Object.values(value).every((item) => isJsonValue(item));
  }

  return false;
}

function humanizeIdentifier(value: string): string {
  return compact(value.split(/[_-]+/g))
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}
