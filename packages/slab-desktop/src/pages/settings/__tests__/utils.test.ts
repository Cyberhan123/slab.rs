import { describe, expect, it } from 'vitest';

import { ApiError } from '@slab/api';

import type { SettingResponse, SettingsSectionResponse } from '../types';
import {
  autoSaveDelay,
  buildRequestBody,
  countProperties,
  countSectionProperties,
  extractStructuredError,
  fallbackErrorMessage,
  matchesSearch,
  parseSettingNumberValue,
  sectionAnchorId,
  shouldCollapseSubsectionHeading,
  subsectionAnchorId,
  summarizeValue,
  valueToEditorString,
} from '../utils';

describe('settings utils', () => {
  it('counts properties and collapses duplicate subsection headings', () => {
    const section = sectionFixture({
      title: ' Runtime ',
      subsections: [
        subsectionFixture({
          title: 'runtime',
          properties: [stringSetting({ pmid: 'runtime.path' }), booleanSetting()],
        }),
      ],
    });

    expect(countProperties([section])).toBe(2);
    expect(countSectionProperties(section)).toBe(2);
    expect(shouldCollapseSubsectionHeading(section, section.subsections[0])).toBe(true);
    expect(
      shouldCollapseSubsectionHeading(
        sectionFixture({
          subsections: [subsectionFixture(), subsectionFixture({ id: 'advanced', title: 'Advanced' })],
        }),
        subsectionFixture(),
      ),
    ).toBe(false);
  });

  it('matches settings search across section, subsection, property, and terms', () => {
    const section = sectionFixture({
      description_md: 'Controls local runtimes',
      subsections: [
        subsectionFixture({
          description_md: 'Model paths',
          properties: [
            stringSetting({
              description_md: 'Where models are stored',
              label: 'Model Directory',
              pmid: 'models.path',
              search_terms: ['cache', 'storage'],
            }),
          ],
        }),
      ],
    });
    const subsection = section.subsections[0];
    const property = subsection.properties[0];

    expect(matchesSearch(section, subsection, property, '')).toBe(true);
    expect(matchesSearch(section, subsection, property, 'storage')).toBe(true);
    expect(matchesSearch(section, subsection, property, 'model directory')).toBe(true);
    expect(matchesSearch(section, subsection, property, 'missing')).toBe(false);
  });

  it('renders editor and summary values defensively', () => {
    expect(valueToEditorString('runtime')).toBe('runtime');
    expect(valueToEditorString(12.5)).toBe('12.5');
    expect(valueToEditorString(Number.NaN)).toBe('');
    expect(valueToEditorString(null)).toBe('');
    expect(valueToEditorString(undefined)).toBe('');
    expect(valueToEditorString({ nested: true })).toBe('{\n  "nested": true\n}');

    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(valueToEditorString(circular)).toBe('');

    expect(summarizeValue('')).toBe('(empty string)');
    expect(summarizeValue('', { emptyString: '（空字符串）' })).toBe('（空字符串）');
    expect(summarizeValue(false)).toBe('false');
    expect(summarizeValue(null)).toBe('null');
    expect(summarizeValue(undefined)).toBe('null');
    expect(summarizeValue({ value: 'x'.repeat(90) })).toHaveLength(80);
  });

  it('extracts structured API validation errors only when the payload is complete', () => {
    expect(
      extractStructuredError(
        new ApiError(4000, 'invalid', {
          message: 'Expected a number',
          path: 'runtime.context_length',
        }),
      ),
    ).toEqual({
      message: 'Expected a number',
      path: 'runtime.context_length',
    });
    expect(extractStructuredError(new ApiError(4000, 'invalid', { message: 'Expected a number' }))).toBeNull();
    expect(extractStructuredError(new Error('invalid'))).toBeNull();
  });

  it('parses complete integer and number strings', () => {
    expect(parseSettingNumberValue('42')).toBe(42);
    expect(parseSettingNumberValue('-7')).toBe(-7);
    expect(parseSettingNumberValue('1.5', 'number')).toBe(1.5);
    expect(parseSettingNumberValue('1e3', 'number')).toBe(1000);
  });

  it('rejects partial numeric strings', () => {
    expect(parseSettingNumberValue('12px')).toBeNull();
    expect(parseSettingNumberValue('1.5')).toBeNull();
    expect(parseSettingNumberValue('1e', 'number')).toBeNull();
    expect(parseSettingNumberValue('Infinity', 'number')).toBeNull();
  });

  it('uses the shared integer parser when building update requests', () => {
    expect(buildRequestBody(integerSetting(), '42')).toEqual({
      op: 'set',
      value: 42,
    });
    expect(buildRequestBody(integerSetting(), '  ')).toEqual({ op: 'unset' });
    expect(() => buildRequestBody(integerSetting(), '42ms')).toThrow(
      'Value must be an integer.',
    );
    expect(() =>
      buildRequestBody(integerSetting(), '42ms', {
        integer: '值必须是整数。',
        json: '值必须是有效的 JSON。',
      }),
    ).toThrow('值必须是整数。');
  });

  it('builds boolean and string request bodies from draft and effective values', () => {
    expect(buildRequestBody(booleanSetting({ effective_value: true }), undefined)).toEqual({
      op: 'set',
      value: true,
    });
    expect(buildRequestBody(booleanSetting({ effective_value: false }), 'true')).toEqual({
      op: 'set',
      value: false,
    });
    expect(buildRequestBody(stringSetting(), '  ')).toEqual({
      op: 'unset',
    });
    expect(buildRequestBody(stringSetting({ schema: { default_value: '', type: 'string' } }), '  ')).toEqual({
      op: 'set',
      value: '  ',
    });
    expect(buildRequestBody(stringSetting({ effective_value: 'current' }), undefined)).toEqual({
      op: 'set',
      value: 'current',
    });
  });

  it('builds structured request bodies from draft objects and JSON strings', () => {
    expect(buildRequestBody(arraySetting(), ['a', 'b'])).toEqual({
      op: 'set',
      value: ['a', 'b'],
    });
    expect(buildRequestBody(objectSetting(), { path: 'models' })).toEqual({
      op: 'set',
      value: { path: 'models' },
    });
    expect(buildRequestBody(objectSetting(), '{"path":"runtime"}')).toEqual({
      op: 'set',
      value: { path: 'runtime' },
    });
    expect(buildRequestBody(objectSetting(), '  ')).toEqual({ op: 'unset' });
    expect(() => buildRequestBody(objectSetting(), '{"path":')).toThrow(
      'Value must be valid JSON.',
    );
    expect(() =>
      buildRequestBody(objectSetting(), '{"path":', {
        integer: '值必须是整数。',
        json: '值必须是有效的 JSON。',
      }),
    ).toThrow('值必须是有效的 JSON。');
  });

  it('falls back to editor strings for unknown setting schema types', () => {
    expect(
      buildRequestBody(
        {
          effective_value: 42,
          schema: {
            default_value: null,
            type: 'custom',
          },
        } as unknown as SettingResponse,
        undefined,
      ),
    ).toEqual({
      op: 'set',
      value: '42',
    });
  });

  it('chooses autosave delays by interaction cost', () => {
    expect(autoSaveDelay(booleanSetting())).toBe(150);
    expect(autoSaveDelay(stringSetting({ schema: { enum: ['auto'], type: 'string' } }))).toBe(150);
    expect(autoSaveDelay(arraySetting())).toBe(900);
    expect(autoSaveDelay(objectSetting())).toBe(900);
    expect(autoSaveDelay(stringSetting({ schema: { multiline: true, type: 'string' } }))).toBe(900);
    expect(autoSaveDelay(stringSetting({ schema: { type: 'string' } }))).toBe(650);
  });

  it('builds stable anchors and delegates fallback error messages', () => {
    expect(sectionAnchorId('runtime')).toBe('settings-section-runtime');
    expect(subsectionAnchorId('runtime', 'paths')).toBe('settings-subsection-runtime-paths');
    expect(fallbackErrorMessage(new ApiError(4004, 'not found'))).toBe('not found');
  });
});

function sectionFixture(overrides: Partial<SettingsSectionResponse> = {}): SettingsSectionResponse {
  return {
    description_md: '',
    id: 'runtime',
    subsections: [subsectionFixture()],
    title: 'Runtime',
    ...overrides,
  } as SettingsSectionResponse;
}

function subsectionFixture(
  overrides: Partial<SettingsSectionResponse['subsections'][number]> = {},
): SettingsSectionResponse['subsections'][number] {
  return {
    description_md: '',
    id: 'general',
    properties: [],
    title: 'Runtime',
    ...overrides,
  } as SettingsSectionResponse['subsections'][number];
}

function settingFixture(overrides: Partial<SettingResponse> = {}): SettingResponse {
  return {
    description_md: '',
    effective_value: '',
    id: 'setting',
    label: 'Setting',
    pmid: 'setting',
    search_terms: [],
    schema: {
      default_value: null,
      type: 'string',
    },
    ...overrides,
  } as SettingResponse;
}

function booleanSetting(overrides: Partial<SettingResponse> = {}): SettingResponse {
  return settingFixture({
    effective_value: false,
    schema: {
      default_value: false,
      type: 'boolean',
    },
    ...overrides,
  });
}

function integerSetting(): SettingResponse {
  return settingFixture({
    effective_value: 0,
    schema: {
      default_value: null,
      type: 'integer',
    },
  });
}

function stringSetting(overrides: Partial<SettingResponse> = {}): SettingResponse {
  return settingFixture({
    effective_value: '',
    schema: {
      default_value: null,
      type: 'string',
    },
    ...overrides,
  });
}

function arraySetting(): SettingResponse {
  return settingFixture({
    effective_value: [],
    schema: {
      default_value: [],
      type: 'array',
    },
  });
}

function objectSetting(): SettingResponse {
  return settingFixture({
    effective_value: {},
    schema: {
      default_value: {},
      type: 'object',
    },
  });
}
