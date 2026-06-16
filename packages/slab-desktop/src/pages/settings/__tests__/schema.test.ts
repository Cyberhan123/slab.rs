import { describe, expect, it } from 'vitest';

import {
  cloneJsonValue,
  createDefaultJsonValue,
  isJsonObject,
  itemSummary,
  jsonPointerAppend,
  parseStructuredJsonSchema,
  pathContainsError,
  schemaAllowsNull,
  schemaFieldDescription,
  schemaFieldLabel,
  schemaFieldPlaceholder,
  schemaPrimaryType,
  type SettingsTranslate,
} from '../schema';
import type { SettingResponse } from '../types';

describe('settings schema helpers', () => {
  it('parses only structured schemas that match the setting type', () => {
    const schema = { type: 'object' as const, properties: { path: { type: 'string' as const } } };

    expect(parseStructuredJsonSchema(setting('object', schema))).toEqual(schema);
    expect(parseStructuredJsonSchema(setting('array', schema))).toBeNull();
    expect(parseStructuredJsonSchema(setting('string', schema))).toBeNull();
    expect(parseStructuredJsonSchema(setting('object', null))).toBeNull();
  });

  it('finds primary types and nullability from JSON schema unions', () => {
    expect(schemaPrimaryType({ type: ['null', 'object'] })).toBe('object');
    expect(schemaPrimaryType({ type: ['null'] })).toBeNull();
    expect(schemaPrimaryType({})).toBeNull();
    expect(schemaAllowsNull({ type: ['string', 'null'] })).toBe(true);
    expect(schemaAllowsNull({ type: 'string' })).toBe(false);
  });

  it('creates safe defaults and deep-clones schema default values', () => {
    const defaultValue = { nested: ['a'] };
    const cloned = createDefaultJsonValue({ type: 'object', default: defaultValue });

    expect(cloned).toEqual(defaultValue);
    expect(cloned).not.toBe(defaultValue);
    expect(createDefaultJsonValue({ type: 'array', minItems: 2, items: { type: 'number' } })).toEqual([
      0,
      0,
    ]);
    expect(createDefaultJsonValue({ type: 'array', minItems: -1, items: { type: 'string' } })).toEqual(
      [],
    );
    expect(createDefaultJsonValue({ type: 'object', properties: { enabled: { type: 'boolean' } } })).toEqual(
      { enabled: false },
    );
    expect(createDefaultJsonValue({ type: 'string' })).toBe('');
  });

  it('clones nested JSON arrays and objects', () => {
    const value = [{ path: 'models', nested: { enabled: true } }];
    const cloned = cloneJsonValue(value);

    expect(cloned).toEqual(value);
    expect(cloned).not.toBe(value);
    expect((cloned as typeof value)[0]).not.toBe(value[0]);
    expect(isJsonObject({})).toBe(true);
    expect(isJsonObject([])).toBe(false);
    expect(isJsonObject(null)).toBe(false);
  });

  it('escapes JSON pointer segments and matches nested error paths', () => {
    expect(jsonPointerAppend('', 'path/with~chars')).toBe('/path~1with~0chars');
    expect(jsonPointerAppend('/root', 0)).toBe('/root/0');
    expect(pathContainsError('/root/name', '/root/name/0')).toBe(true);
    expect(pathContainsError('/root/name', '/root/other')).toBe(false);
    expect(pathContainsError('/root/name')).toBe(false);
  });

  it('builds labels, placeholders, and item summaries from schema metadata', () => {
    expect(schemaFieldLabel('remote_model', { title: 'Remote model' })).toBe('Remote model');
    expect(schemaFieldLabel('remote_model', {})).toBe('Remote Model');
    expect(schemaFieldPlaceholder({ examples: ['llama.cpp'], title: 'Remote model' })).toBe(
      'llama.cpp',
    );
    expect(schemaFieldPlaceholder({ title: 'Remote model' })).toBe('Enter remote model');
    expect(schemaFieldPlaceholder({})).toBeUndefined();
    expect(itemSummary({ display_name: ' Model ', id: 'fallback' })).toBe('Model');
    expect(itemSummary({ other: 'value' })).toBeNull();
    expect(itemSummary('value')).toBeNull();
  });

  it('translates schema labels, descriptions, and placeholders from server i18n metadata', () => {
    const t = testTranslate({
      'server.settings.schemas.mcp.name.title': '服务器名称',
      'server.settings.schemas.mcp.name.description': '用于路由 MCP 工具调用的稳定本地名称。',
      'pages.settings.field.enterNamedValue': '输入{{label}}',
    });
    const schema = {
      type: 'string' as const,
      title: 'Server Name',
      description: 'Stable local name used to route MCP tool calls.',
      'x-i18n': {
        title: {
          key: 'server.settings.schemas.mcp.name.title',
        },
        description: {
          key: 'server.settings.schemas.mcp.name.description',
        },
      },
    };

    expect(schemaFieldLabel('name', schema, t)).toBe('服务器名称');
    expect(schemaFieldDescription(schema, t)).toBe('用于路由 MCP 工具调用的稳定本地名称。');
    expect(schemaFieldPlaceholder(schema, t)).toBe('输入服务器名称');
  });
});

function setting(type: SettingResponse['schema']['type'], json_schema: unknown): SettingResponse {
  return {
    schema: {
      json_schema,
      type,
    },
  } as SettingResponse;
}

function testTranslate(messages: Record<string, string>): SettingsTranslate {
  return ((key: string, options?: Record<string, unknown>) => {
    let message = messages[key] ?? key;
    for (const [name, value] of Object.entries(options ?? {})) {
      message = message.replaceAll(`{{${name}}}`, String(value));
    }
    return message;
  }) as SettingsTranslate;
}
