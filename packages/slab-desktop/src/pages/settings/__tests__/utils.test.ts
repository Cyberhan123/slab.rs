import { describe, expect, it } from 'vitest';

import type { SettingResponse } from '../types';
import { buildRequestBody, parseSettingNumberValue } from '../utils';

describe('settings utils', () => {
  it('parses complete integer and number strings', () => {
    expect(parseSettingNumberValue('42', 'integer')).toBe(42);
    expect(parseSettingNumberValue('-7', 'integer')).toBe(-7);
    expect(parseSettingNumberValue('1.5', 'number')).toBe(1.5);
    expect(parseSettingNumberValue('1e3', 'number')).toBe(1000);
  });

  it('rejects partial numeric strings', () => {
    expect(parseSettingNumberValue('12px', 'integer')).toBeNull();
    expect(parseSettingNumberValue('1.5', 'integer')).toBeNull();
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
  });
});

function integerSetting(): SettingResponse {
  return {
    effective_value: 0,
    schema: {
      default_value: null,
      type: 'integer',
    },
  } as SettingResponse;
}
