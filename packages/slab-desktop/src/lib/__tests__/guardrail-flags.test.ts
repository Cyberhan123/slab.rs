import { describe, expect, it } from 'vitest';

import { settingValueToEnabledFlag } from '../guardrail-flags';

describe('guardrail flags', () => {
  it('defaults guardrail settings to enabled unless a boolean override is present', () => {
    expect(settingValueToEnabledFlag(true)).toBe(true);
    expect(settingValueToEnabledFlag(false)).toBe(false);
    expect(settingValueToEnabledFlag(null)).toBe(true);
    expect(settingValueToEnabledFlag(undefined)).toBe(true);
    expect(settingValueToEnabledFlag('false')).toBe(true);
  });
});
